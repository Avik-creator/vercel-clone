use axum::{
    extract::FromRequestParts,
    http::{request::Parts},
    RequestPartsExt,
};
use axum_extra::{
    headers::{Authorization, authorization::Bearer},
    TypedHeader,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;
use crate::{
    AppState,
    errors::AppError,
    models::User,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,   // user ID
    pub exp: i64,
    pub iat: i64,
}

// Query params for token (used by SSE endpoints)
#[derive(Debug, Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

pub struct AuthUser(pub User);

impl FromRequestParts<AppState> for AuthUser {
  type Rejection = AppError;

  async fn from_request_parts(
    req: &mut Parts,
    state: &AppState,
  ) -> Result<Self, Self::Rejection> {
    // First try Authorization header
    if let Ok(TypedHeader(Authorization(bearer))) =
            req.extract::<TypedHeader<Authorization<Bearer>>>().await
        {
            let token = bearer.token();

            // Check if it looks like an API key (prefix cp_)
            if token.starts_with("cp_") {
                return authenticate_api_key(token, state).await;
            }

            return authenticate_jwt(token, state).await;
        }

    // Fallback to query parameter (for SSE endpoints)
    if let Ok(axum::extract::Query(query)) = req.extract::<axum::extract::Query<TokenQuery>>().await {
        if let Some(token) = query.token {
            if token.starts_with("cp_") {
                return authenticate_api_key(&token, state).await;
            }
            return authenticate_jwt(&token, state).await;
        }
    }

    Err(AppError::Unauthorized("missing authorization header".into()))
  }
}

async fn authenticate_jwt(token: &str, state: &AppState) -> Result<AuthUser, AppError> {
    let key = DecodingKey::from_secret(state.config.jwt_secret.as_bytes());
    let claims = decode::<Claims>(token, &key, &Validation::default())
        .map_err(|_| AppError::Unauthorized("invalid or expired token".into()))?
        .claims;

    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1"
    )
    .bind(claims.sub)
    .fetch_one(&*state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::Unauthorized("user not found".into()),
        _ => AppError::Database(e.into()),
    })?;

    Ok(AuthUser(user))
}

async fn authenticate_api_key(token: &str, state: &AppState) -> Result<AuthUser, AppError> {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};

    // Look up by prefix (first 16 chars) for efficiency, then verify full hash
    let prefix = &token[..token.len().min(20)];

    let row = sqlx::query(
        r#"
        SELECT ak.id as api_key_id, ak.key_hash, ak.expires_at, u.id as user_id,
               u.email, u.name, u.github_id, u.github_login,
               u.password_hash, u.created_at, u.updated_at
        FROM api_keys ak
        JOIN users u ON ak.user_id = u.id
        WHERE ak.key_prefix = $1
        "#,
    )
    .bind(prefix)
    .fetch_one(&*state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::Unauthorized("invalid api key".into()),
        _ => AppError::Database(e.into()),
    })?;

    let api_key_id: Uuid = row.try_get("api_key_id")?;
    let key_hash: String = row.try_get("key_hash")?;
    let expires_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("expires_at")?;
    let user_id: Uuid = row.try_get("user_id")?;
    let email: String = row.try_get("email")?;
    let name: String = row.try_get("name")?;
    let github_id: Option<i64> = row.try_get("github_id")?;
    let github_login: Option<String> = row.try_get("github_login")?;
    let password_hash: Option<String> = row.try_get("password_hash")?;
    let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
    let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

    // Check expiry
    if let Some(exp) = expires_at {
        if exp < chrono::Utc::now() {
            return Err(AppError::Unauthorized("api key expired".into()));
        }
    }

     // Verify full hash
    let hash = PasswordHash::new(&key_hash)
        .map_err(|_| AppError::Unauthorized("invalid api key".into()))?;
    Argon2::default()
        .verify_password(token.as_bytes(), &hash)
        .map_err(|_| AppError::Unauthorized("invalid api key".into()))?;

    // Update last_used_at async (fire and forget)
    let pool = state.db.pool.clone();
    tokio::spawn(async move {
        let _ = sqlx::query(
            "UPDATE api_keys SET last_used_at = NOW() WHERE id = $1",
        )
        .bind(api_key_id)
        .execute(&pool)
        .await;
    });

    let user = User {
        id: user_id,
        email,
        name,
        password_hash,
        github_id,
        github_login,
        created_at,
        updated_at,
    };

    Ok(AuthUser(user))

}

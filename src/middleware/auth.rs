use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
    RequestPartsExt,
};
use axum_extra::{
    headers::{Authorization, authorization::Bearer},
    TypedHeader,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{
    AppState,
    errors::AppError,
    models::user::User,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: Uuid,   // user ID
    pub exp: i64,
    pub iat: i64,
}

pub struct AuthUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
  type Rejection = AppError;

  async fn from_request_parts(
    req: &mut Parts,
    state: &AppState,
  ) -> Result<Self, Self::Rejection> {
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

    let row = sqlx::query!(
        r#"
        SELECT ak.id as api_key_id, ak.key_hash, ak.expires_at, u.id as user_id,
               u.email, u.name, u.github_id, u.github_login,
               u.password_hash, u.created_at, u.updated_at
        FROM api_keys ak
        JOIN users u ON ak.user_id = u.id
        WHERE ak.key_prefix = $1
        "#,
        prefix
    )
    .fetch_one(&*state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => AppError::Unauthorized("invalid api key".into()),
        _ => AppError::Database(e.into()),
    })?;

    // Check expiry
    if let Some(exp) = row.expires_at {
        if exp < chrono::Utc::now() {
            return Err(AppError::Unauthorized("api key expired".into()));
        }
    }

     // Verify full hash
    let hash = PasswordHash::new(&row.key_hash)
        .map_err(|_| AppError::Unauthorized("invalid api key".into()))?;
    Argon2::default()
        .verify_password(token.as_bytes(), &hash)
        .map_err(|_| AppError::Unauthorized("invalid api key".into()))?;

    // Update last_used_at async (fire and forget)
    let pool = state.db.pool.clone();
    let api_key_id = row.api_key_id;
    tokio::spawn(async move {
        let _ = sqlx::query!(
            "UPDATE api_keys SET last_used_at = NOW() WHERE id = $1",
            api_key_id
        )
        .execute(&pool)
        .await;
    });

    let user = User {
        id: row.user_id,
        email: row.email,
        name: row.name,
        password_hash: row.password_hash,
        github_id: row.github_id,
        github_login: row.github_login,
        created_at: row.created_at,
        updated_at: row.updated_at,
    };

    Ok(AuthUser(user))

}

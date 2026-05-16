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
            parts.extract::<TypedHeader<Authorization<Bearer>>>().await
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
    .map_err(|_| AppError::Unauthorized("user not found".into()))?;

    Ok(AuthUser(user))
}


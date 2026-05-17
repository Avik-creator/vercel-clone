use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, PasswordHash, PasswordVerifier,
};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use uuid::Uuid;

use crate::{
    AppState,
    errors::{AppError, AppResult},
    middleware::auth::Claims,
    models::{AuthResponse, CreateUserRequest, LoginRequest, User},
};


const TOKEN_EXPIRY_SECS: i64 = 60 * 60 * 24; // 24 hours

pub async fn register(state: &AppState, body: CreateUserRequest) -> AppResult<AuthResponse> {
    // Check email not taken
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)"
    )
    .bind(&body.email)
    .fetch_one(&*state.db)
    .await?;

    if exists {
        return Err(AppError::Conflict("email already registered".into()));
    }

    let password_hash = hash_password(&body.password)?;
    let now = Utc::now();

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (email, name, password_hash, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $4)
        RETURNING *
        "#
    )
    .bind(&body.email)
    .bind(&body.name)
    .bind(&password_hash)
    .bind(now)
    .fetch_one(&*state.db)
    .await?;

    let token = mint_jwt(&user.id, &state.config.jwt_secret)?;
    Ok(AuthResponse { token, user })
}

pub async fn login(state: &AppState, req: LoginRequest) -> AppResult<AuthResponse> {
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE email = $1"
    )
    .bind(&req.email)
    .fetch_optional(&*state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("invalid credentials".into()))?;

    let hash = user.password_hash.as_deref()
        .ok_or_else(|| AppError::Unauthorized("use github login".into()))?;

    verify_password(&req.password, hash)?;

    let token = mint_jwt(&user.id, &state.config.jwt_secret)?;
    Ok(AuthResponse { token, user })
}

pub fn mint_jwt(user_id: &Uuid, secret: &str) -> AppResult<String> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: *user_id,
        iat: now,
        exp: now + TOKEN_EXPIRY_SECS,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("jwt error: {e}")))
}


pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(anyhow::anyhow!("hash error: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> AppResult<()> {
    let parsed = PasswordHash::new(hash)
        .map_err(|_| AppError::Unauthorized("invalid credentials".into()))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| AppError::Unauthorized("invalid credentials".into()))
}

pub async fn github_oauth(state: &AppState, code: &str) -> AppResult<AuthResponse> {
    // Exchange code for access token via reqwest
    let mut form_data = std::collections::HashMap::<&str, &str>::new();
    form_data.insert("client_id", state.config.github_client_id.as_str());
    form_data.insert("client_secret", state.config.github_client_secret.as_str());
    form_data.insert("code", code);

    let token_resp = reqwest::Client::new()
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&form_data)
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("github token request failed: {e}")))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("github token parse failed: {e}")))?;

    let access_token = token_resp["access_token"].as_str()
        .ok_or_else(|| AppError::BadRequest(format!("github did not return access_token: {token_resp}")))?;

    // Fetch user info via octocrab
    let octocrab = octocrab::OctocrabBuilder::new()
        .personal_token(access_token.to_string())
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("octocrab build failed: {e}")))?;

    let user = octocrab.current().user().await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("github user fetch failed: {e}")))?;

    let github_id = user.id.0 as i64;
    let github_login = user.login.clone();
    let name = user.name.clone().unwrap_or_else(|| github_login.clone());

    let email = user.email.unwrap_or_else(|| {
        format!("{}@users.noreply.github.com", github_login)
    });

    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (email, name, github_id, github_login, created_at, updated_at)
        VALUES ($1, $2, $3, $4, NOW(), NOW())
        ON CONFLICT (email)
        DO UPDATE SET
            github_id = EXCLUDED.github_id,
            github_login = EXCLUDED.github_login,
            updated_at = NOW()
        RETURNING *
        "#
    )
    .bind(&email)
    .bind(&name)
    .bind(github_id)
    .bind(&github_login)
    .fetch_one(&*state.db)
    .await?;

    let token = mint_jwt(&user.id, &state.config.jwt_secret)?;
    Ok(AuthResponse { token, user })
}

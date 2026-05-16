use axum::{extract::State, Json};
use crate::{
    AppState,
    errors::{AppError, AppResult},
    models::{CreateUserRequest, LoginRequest, AuthResponse},
    services::auth as auth_service,
};

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> AppResult<Json<AuthResponse>> {
    let resp = auth_service::register(&state, body).await?;
    Ok(Json(resp))
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    let resp = auth_service::login(&state, body).await?;
    Ok(Json(resp))
}

pub async fn refresh(
    State(_state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    Err(AppError::BadRequest("not yet implemented".into()))
}

pub async fn github_oauth_redirect(
    State(_state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    Err(AppError::BadRequest("not yet implemented".into()))
}

pub async fn github_oauth_callback(
    State(_state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    Err(AppError::BadRequest("not yet implemented".into()))
}

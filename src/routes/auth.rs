use axum::{
    extract::{Query, State},
    response::Redirect,
    Json,
};
use serde::Deserialize;
use crate::{
    AppState,
    errors::{AppError, AppResult},
    models::{CreateUserRequest, LoginRequest, AuthResponse},
    services::auth as auth_service,
};

#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

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
    State(state): State<AppState>,
) -> Redirect {
    let mut url = url::Url::parse("https://github.com/login/oauth/authorize").unwrap();
    url.query_pairs_mut()
        .append_pair("client_id", &state.config.github_client_id)
        .append_pair("redirect_uri", &format!("{}/v1/auth/github/callback", state.config.base_domain))
        .append_pair("scope", "read:user user:email");
    Redirect::to(url.as_str())
}

pub async fn github_oauth_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> AppResult<Json<AuthResponse>> {
    let resp = auth_service::github_oauth(&state, &query.code).await?;
    Ok(Json(resp))
}

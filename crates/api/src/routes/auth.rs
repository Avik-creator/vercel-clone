use axum::{
    extract::{Query, State},
    response::Redirect,
    Json,
};
use serde::Deserialize;
use crate::{
    AppState,
    errors::{AppError, AppResult},
    middleware::auth::AuthUser,
    models::{AuthResponse, CreateUserRequest, LoginRequest, User},
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
) -> AppResult<Redirect> {
    let resp = auth_service::github_oauth(&state, &query.code).await?;
    let redirect_url = format!("{}/auth/callback?token={}", state.config.frontend_url, resp.token);
    Ok(Redirect::to(&redirect_url))
}

pub async fn me(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<User>> {
    // The AuthUser middleware already fetched the user from the token
    // Return the current user
    let user = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = $1"
    )
    .bind(user.id)
    .fetch_one(&*state.db)
    .await?;
    
    Ok(Json(user))
}

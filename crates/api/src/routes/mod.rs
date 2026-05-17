use crate::AppState;
use axum::{
    Router,
    routing::{delete, get, post},
};

pub mod api_keys;
pub mod auth;
pub mod deployments;
pub mod github;
pub mod health;
pub mod projects;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/ready", get(health::readiness_check))
        .route("/v1/auth/register", post(auth::register))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/refresh", post(auth::refresh))
        .route("/v1/auth/me", get(auth::me))
        .route("/v1/auth/github", get(auth::github_oauth_redirect))
        .route("/v1/auth/github/callback", get(auth::github_oauth_callback))
        .route("/v1/projects", get(projects::list).post(projects::create))
        .route(
            "/v1/projects/{id}",
            get(projects::get)
                .patch(projects::update)
                .delete(projects::delete),
        )
        .route(
            "/v1/projects/{id}/env",
            get(projects::get_env)
                .put(projects::set_env)
                .post(projects::add_env),
        )
        .route("/v1/projects/{id}/env/{key}", delete(projects::delete_env))
        .route("/v1/projects/{id}/link", post(projects::link_github))
        .route("/v1/deployments", get(deployments::list))
        .route("/v1/deployments/{id}", get(deployments::get))
        .route("/v1/deployments/{id}/cancel", post(deployments::cancel))
        .route("/v1/deployments/{id}/promote", post(deployments::promote))
        .route("/v1/deployments/{id}/logs", get(deployments::stream_logs))
        .route(
            "/v1/projects/{id}/deployments",
            get(deployments::list_for_project).post(deployments::create),
        )
        .route("/v1/api-keys", get(api_keys::list).post(api_keys::create))
        .route("/v1/api-keys/{id}", delete(api_keys::revoke))
        .route("/webhooks/github", post(github::handle_webhook))
        .route("/v1/github/repos", get(github::list_repos))
        .route(
            "/internal/builds/callback",
            post(deployments::build_callback),
        )
        .fallback(deployments::serve_artifact)
        .with_state(state)
}

use axum::{
    body::Bytes,
    extract::State,
    http::HeaderMap,
    Json,
};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use crate::{
    AppState,
    errors::{AppError, AppResult},
    middleware::auth::AuthUser,
    services::github as github_service,
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Serialize)]
pub struct GitHubRepo {
    pub id: i64,
    pub name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub private: bool,
    pub default_branch: String,
    pub html_url: String,
}

pub async fn list_repos(
    State(_state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Vec<GitHubRepo>>> {
    let access_token = user.github_access_token
        .ok_or_else(|| AppError::BadRequest("GitHub account not linked. Please sign in with GitHub.".into()))?;

    let octocrab = octocrab::OctocrabBuilder::new()
        .personal_token(access_token)
        .build()
        .map_err(|e| AppError::Internal(anyhow::anyhow!("octocrab build failed: {e}")))?;

    // Fetch user repos - get first 100 repos sorted by recently pushed
    let repos = octocrab
        .current()
        .list_repos_for_authenticated_user()
        .sort("pushed")
        .direction("desc")
        .per_page(100)
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("github repos fetch failed: {e}")))?;

    let repos: Vec<GitHubRepo> = repos.items.into_iter().map(|r| GitHubRepo {
        id: r.id.0 as i64,
        name: r.name,
        full_name: r.full_name.unwrap_or_default(),
        description: r.description,
        private: r.private.unwrap_or(false),
        default_branch: r.default_branch.unwrap_or_else(|| "main".to_string()),
        html_url: r.html_url.map(|u| u.to_string()).unwrap_or_default(),
    }).collect();

    Ok(Json(repos))
}

pub async fn handle_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<serde_json::Value>> {
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("sha256="))
        .ok_or(AppError::InvalidWebhookSignature)?;

    verify_signature(&state.config.github_webhook_secret, &body, signature)?;

    let event_type = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    let delivery_id = headers
        .get("x-github-delivery")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    tracing::info!(event = %event_type, delivery = %delivery_id, "github webhook received");

    let payload: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("invalid JSON: {e}")))?;

    match event_type {
        "push" => github_service::handle_push(&state, payload).await?,
        "pull_request" => github_service::handle_pull_request(&state, payload).await?,
        "installation" => github_service::handle_installation(&state, payload).await?,
        "installation_repositories" => github_service::handle_installation_repositories(&state, payload).await?,
        "ping" => tracing::info!("github ping received"),
        other => tracing::debug!(event = %other, "unhandled github event"),
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

fn verify_signature(secret: &str, body: &[u8], signature: &str) -> AppResult<()> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| AppError::Internal(anyhow::anyhow!("hmac key error")))?;
    mac.update(body);
    let expected = hex::encode(mac.finalize().into_bytes());

    if expected != signature {
        return Err(AppError::InvalidWebhookSignature);
    }
    Ok(())
}

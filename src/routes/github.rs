use axum::{
    body::Bytes,
    extract::State,
    http::HeaderMap,
    Json,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use crate::{
    AppState,
    errors::{AppError, AppResult},
    services::github as github_service,
};

type HmacSha256 = Hmac<Sha256>;

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

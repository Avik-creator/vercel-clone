use crate::{
    AppState, errors::AppResult, middleware::auth::AuthUser, models::CreateApiKeyRequest,
    services::api_keys as key_service,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::Value;
use uuid::Uuid;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let keys = key_service::list(&state, user.id).await?;
    Ok(Json(serde_json::to_value(keys).unwrap()))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateApiKeyRequest>,
) -> AppResult<Json<Value>> {
    let key = key_service::create(&state, user.id, body).await?;
    Ok(Json(serde_json::to_value(key).unwrap()))
}

pub async fn revoke(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    key_service::revoke(&state, user.id, id).await?;
    Ok(Json(serde_json::json!({ "revoked": true })))
}

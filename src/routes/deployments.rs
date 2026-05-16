use axum::{
    extract::{Path, State},
    Json,
    response::sse::{Event, Sse},
};
use futures::stream;
use serde_json::Value;
use uuid::Uuid;
use crate::{
    AppState,
    errors::{AppError, AppResult},
    middleware::auth::AuthUser,
    models::{CreateDeploymentRequest, BuildCallbackRequest},
    services::deployments as deploy_service,
};

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let deploys = deploy_service::list_for_user(&state, user.id).await?;
    Ok(Json(serde_json::to_value(deploys).unwrap()))
}

pub async fn list_for_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let deploys = deploy_service::list_for_project(&state, user.id, project_id).await?;
    Ok(Json(serde_json::to_value(deploys).unwrap()))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(mut body): Json<CreateDeploymentRequest>,
) -> AppResult<Json<Value>> {
    body.project_id = Some(project_id);
    let deploy = deploy_service::create(&state, user.id, body).await?;
    Ok(Json(serde_json::to_value(deploy).unwrap()))
}

pub async fn get(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let deploy = deploy_service::get_for_user(&state, user.id, id).await?;
    Ok(Json(serde_json::to_value(deploy).unwrap()))
}

pub async fn cancel(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    deploy_service::cancel(&state, user.id, id).await?;
    Ok(Json(serde_json::json!({ "cancelled": true })))
}

pub async fn promote(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    deploy_service::promote_to_production(&state, user.id, id).await?;
    Ok(Json(serde_json::json!({ "promoted": true })))
}

pub async fn stream_logs(
    State(_state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(_id): Path<Uuid>,
) -> Sse<impl futures::Stream<Item = Result<Event, std::convert::Infallible>>> {
    let placeholder = stream::iter(vec![
        Ok(Event::default().data("log streaming not yet implemented")),
    ]);
    Sse::new(placeholder)
}

pub async fn build_callback(
    State(state): State<AppState>,
    axum_extra::TypedHeader(auth): axum_extra::TypedHeader<axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>>,
    Json(body): Json<BuildCallbackRequest>,
) -> AppResult<Json<Value>> {
    if auth.token() != state.config.build_worker_secret {
        return Err(AppError::Unauthorized("invalid worker secret".into()));
    }
    deploy_service::handle_build_callback(&state, body).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

use axum::{
    extract::{Path, State},
    Json,
    response::sse::{Event, Sse},
};
use futures::Stream;
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
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(id): Path<Uuid>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let sender = state.nats.get_log_sender(id).await;
    let mut receiver = sender.subscribe();

    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(log_line) => {
                    yield Ok(Event::default().data(format!("{}: {}", log_line.timestamp.format("%H:%M:%S"), log_line.line)));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    yield Ok(Event::default().data(format!("[lagged: {} lines dropped]", n)));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Sse::new(stream)
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

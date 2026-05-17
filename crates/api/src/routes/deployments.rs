use crate::{
    AppState,
    errors::{AppError, AppResult},
    middleware::auth::AuthUser,
    models::{BuildCallbackRequest, CreateDeploymentRequest},
    services::deployments as deploy_service,
};
use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
    response::sse::{Event, Sse},
};
use futures::Stream;
use serde_json::Value;
use uuid::Uuid;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let deploys = deploy_service::list_for_user(&state, user.id).await?;
    Ok(Json(to_json_value(deploys)?))
}

pub async fn list_for_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let deploys = deploy_service::list_for_project(&state, user.id, project_id).await?;
    Ok(Json(to_json_value(deploys)?))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(mut body): Json<CreateDeploymentRequest>,
) -> AppResult<Json<Value>> {
    body.project_id = Some(project_id);
    let deploy = deploy_service::create(&state, user.id, body).await?;
    Ok(Json(to_json_value(deploy)?))
}

pub async fn get(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let deploy = deploy_service::get_for_user(&state, user.id, id).await?;
    Ok(Json(to_json_value(deploy)?))
}

fn to_json_value<T: serde::Serialize>(value: T) -> AppResult<Value> {
    serde_json::to_value(value).map_err(|e| AppError::Internal(e.into()))
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
    axum_extra::TypedHeader(auth): axum_extra::TypedHeader<
        axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>,
    >,
    Json(body): Json<BuildCallbackRequest>,
) -> AppResult<Json<Value>> {
    if auth.token() != state.config.build_worker_secret {
        return Err(AppError::Unauthorized("invalid worker secret".into()));
    }
    deploy_service::handle_build_callback(&state, body).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn serve_artifact(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::OriginalUri(uri): axum::extract::OriginalUri,
) -> AppResult<Response> {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(':').next().unwrap_or(value))
        .ok_or_else(|| AppError::NotFound("deployment not found".into()))?;

    let artifact_key = sqlx::query_scalar::<_, String>(
        "SELECT artifact_key FROM deployments WHERE url = $1 AND state = 'ready' AND artifact_key IS NOT NULL",
    )
    .bind(host)
    .fetch_optional(&*state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("deployment not found".into()))?;

    let object_key = deploy_service::artifact_object_key(&artifact_key, uri.path());
    let minio_url = format!(
        "{}/{}/{}",
        state.config.minio_endpoint.trim_end_matches('/'),
        state.config.minio_bucket,
        object_key
    );

    let object = reqwest::get(minio_url)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to fetch artifact: {}", e)))?;

    if object.status() == StatusCode::NOT_FOUND {
        return Err(AppError::NotFound("artifact not found".into()));
    }

    let status = object.status();
    let content_type = object.headers().get(header::CONTENT_TYPE).cloned();
    let bytes = object
        .bytes()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to read artifact: {}", e)))?;

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = status;
    if let Some(content_type) = content_type {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, content_type);
    }

    Ok(response)
}

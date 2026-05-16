use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::Value;
use uuid::Uuid;
use crate::{
    AppState,
    errors::{AppError, AppResult},
    middleware::auth::AuthUser,
    models::{CreateProjectRequest, UpdateProjectRequest},
    services::projects as project_service,
};

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Json<Value>> {
    let projects = project_service::list_for_user(&state, user.id).await?;
    Ok(Json(serde_json::to_value(projects).unwrap()))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateProjectRequest>,
) -> AppResult<Json<Value>> {
    let project = project_service::create(&state, user.id, body).await?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

pub async fn get(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let project = project_service::get_for_user(&state, user.id, id).await?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

pub async fn update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProjectRequest>,
) -> AppResult<Json<Value>> {
    let project = project_service::update(&state, user.id, id, body).await?;
    Ok(Json(serde_json::to_value(project).unwrap()))
}

pub async fn delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    project_service::delete(&state, user.id, id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn get_env(
    State(_state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    Err(AppError::BadRequest("not yet implemented".into()))
}

pub async fn set_env(
    State(_state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    Err(AppError::BadRequest("not yet implemented".into()))
}

pub async fn link_github(
    State(_state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    Err(AppError::BadRequest("not yet implemented".into()))
}

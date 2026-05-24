use crate::{
    AppState,
    errors::{AppError, AppResult},
    services::admin::{self, ReplayResponse},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct ListFailedJobsQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

pub async fn list_failed_jobs(
    State(state): State<AppState>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Query(query): Query<ListFailedJobsQuery>,
) -> AppResult<Json<Value>> {
    admin::verify_admin_secret(&state.config, auth.token())?;
    let jobs = admin::list_failed_jobs(&state.nats, query.limit).await?;
    Ok(Json(serde_json::to_value(jobs).map_err(|e| AppError::Internal(e.into()))?))
}

pub async fn replay_failed_job(
    State(state): State<AppState>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    Path(sequence): Path<u64>,
) -> AppResult<Json<ReplayResponse>> {
    admin::verify_admin_secret(&state.config, auth.token())?;
    let response = admin::replay_failed_job(&state.nats, sequence).await?;
    Ok(Json(response))
}

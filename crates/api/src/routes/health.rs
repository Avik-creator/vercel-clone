use axum::{extract::State, Json};
use serde_json::{json, Value};
use crate::AppState;

pub async fn health_check() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn readiness_check(State(state): State<AppState>) -> Json<Value> {
    match sqlx::query("SELECT 1").execute(&*state.db).await {
        Ok(_) => Json(json!({ "status": "ready", "db": "ok" })),
        Err(e) => Json(json!({ "status": "degraded", "db": e.to_string() })),
    }
}

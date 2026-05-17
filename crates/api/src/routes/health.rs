use crate::AppState;
use axum::{Json, extract::State};
use serde_json::{Value, json};

pub async fn health_check() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn readiness_check(State(state): State<AppState>) -> Json<Value> {
    match sqlx::query("SELECT 1").execute(&*state.db).await {
        Ok(_) => Json(json!({ "status": "ready", "db": "ok" })),
        Err(e) => Json(json!({ "status": "degraded", "db": e.to_string() })),
    }
}

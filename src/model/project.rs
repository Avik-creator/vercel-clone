use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub slug: String,
    /// GitHub repo full name e.g. "alice/my-app"
    pub github_repo: Option<String>,
    /// GitHub App installation ID for this repo
    pub github_installation_id: Option<i64>,
    pub framework: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub root_dir: Option<String>,
    pub env_vars: serde_json::Value,   // encrypted JSON blob
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub github_repo: Option<String>,
    pub framework: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub root_dir: Option<String>,
}

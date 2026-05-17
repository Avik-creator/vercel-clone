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
    pub production_branch: String,
    pub env_vars: serde_json::Value,   // encrypted JSON blob
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarEntry {
    pub key: String,
    pub value: String,
    pub target: EnvVarTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EnvVarTarget {
    Build,
    Runtime,
    All,
}

#[derive(Debug, Deserialize)]
pub struct CreateEnvVarRequest {
    pub key: String,
    pub value: String,
    pub target: Option<EnvVarTarget>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEnvVarsRequest {
    pub env_vars: Vec<EnvVarEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub github_repo: Option<String>,
    pub framework: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub production_branch: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub root_dir: Option<String>,
    pub production_branch: Option<String>,
}

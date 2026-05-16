use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "deployment_state", rename_all = "lowercase")]
pub enum DeploymentState {
    Queued,
    Building,
    Uploading,
    Ready,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Deployment {
    pub id: Uuid,
    pub project_id: Uuid,
    /// Git commit SHA that triggered this deploy
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub branch: String,
    pub state: DeploymentState,
    /// Preview URL slug e.g. "abc123-my-app.yourdomain.app"
    pub url: Option<String>,
    /// Whether this is the production deployment
    pub is_production: bool,
    pub build_log: Option<String>,
    pub build_started_at: Option<DateTime<Utc>>,
    pub build_finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeploymentRequest {
    pub project_id: Option<Uuid>,
    pub commit_sha: String,
    pub commit_message: Option<String>,
    pub branch: String,
}

#[derive(Debug, Deserialize)]
pub struct BuildCallbackRequest {
    pub deployment_id: Uuid,
    pub state: DeploymentState,
    pub log_chunk: Option<String>,
    pub artifact_url: Option<String>,
}

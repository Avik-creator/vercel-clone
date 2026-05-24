use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentState {
    Queued,
    Building,
    Uploading,
    Ready,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildJob {
    pub deployment_id: Uuid,
    pub project_id: Uuid,
    pub git_url: String,
    pub commit_sha: String,
    pub branch: String,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub github_token: Option<String>,
    pub env_vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub deployment_id: Uuid,
    pub state: DeploymentState,
    pub artifact_key: Option<String>,
    pub image_ref: Option<String>,
    pub log_output: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogLine {
    pub deployment_id: Uuid,
    pub line: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedBuildJob {
    pub job: BuildJob,
    pub error: String,
    pub failed_at: DateTime<Utc>,
}

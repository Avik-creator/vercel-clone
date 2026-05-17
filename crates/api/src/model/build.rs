use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Payload sent to a build worker via the queue
#[derive(Debug, Serialize, Deserialize, Clone)]
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

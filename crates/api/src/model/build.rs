use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Payload sent to a build worker via the queue
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildJob {
    pub deployment_id: Uuid,
    pub project_id: Uuid,
    pub repo_clone_url: String,
    pub commit_sha: String,
    pub branch: String,
    pub framework: Option<String>,
    pub build_command: String,
    pub output_dir: String,
    pub env_vars: Vec<EnvVar>,
    pub callback_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

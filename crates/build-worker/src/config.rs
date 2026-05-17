use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct WorkerConfig {
    #[serde(default = "default_nats_url")]
    pub nats_url: String,

    #[serde(default = "default_minio_endpoint")]
    pub minio_endpoint: String,

    pub minio_access_key: String,
    pub minio_secret_key: String,

    #[serde(default = "default_minio_bucket")]
    pub minio_bucket: String,

    #[serde(default = "default_docker_network")]
    pub docker_network: String,

    #[serde(default = "default_build_timeout_secs")]
    pub build_timeout_secs: u64,
}

impl WorkerConfig {
    pub fn load() -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();
        let cfg = config::Config::builder()
            .add_source(config::Environment::default().separator("__"))
            .build()?
            .try_deserialize()?;
        Ok(cfg)
    }
}

fn default_nats_url() -> String {
    "nats://localhost:4222".into()
}
fn default_minio_endpoint() -> String {
    "http://localhost:9000".into()
}
fn default_minio_bucket() -> String {
    "deployments".into()
}
fn default_docker_network() -> String {
    "vercel-clone_default".into()
}
fn default_build_timeout_secs() -> u64 {
    600
}

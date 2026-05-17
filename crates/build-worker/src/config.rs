use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct WorkerConfig {
    #[serde(default = "default_nats_url")]
    pub nats_url: String,

    pub nats_user: Option<String>,
    pub nats_password: Option<String>,

    /// Registry URL as seen by Docker daemon (host port-forwarded, used in image_ref stored in DB).
    #[serde(default = "default_registry_url")]
    pub registry_url: String,

    /// Registry URL as seen by BuildKit daemon (Docker-network DNS, used for direct push).
    /// Defaults to registry_url when not set.
    pub build_registry_url: Option<String>,

    #[serde(default = "default_build_network")]
    pub build_network: String,

    #[serde(default = "default_build_timeout_secs")]
    pub build_timeout_secs: u64,

    #[serde(default = "default_max_concurrent_builds")]
    pub max_concurrent_builds: usize,
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
fn default_registry_url() -> String {
    "localhost:5000".into()
}
fn default_build_network() -> String {
    "vercel-clone_build-net".into()
}
fn default_build_timeout_secs() -> u64 {
    600
}
fn default_max_concurrent_builds() -> usize {
    2
}

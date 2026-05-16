use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_env")]
    pub env: String,

    pub database_url: String,

    pub jwt_secret: String,

    /// GitHub App credentials
    pub github_app_id: u64,
    pub github_app_private_key: String,
    pub github_webhook_secret: String,

    /// Internal secret for build worker callbacks
    pub build_worker_secret: String,

    #[serde(default = "default_base_domain")]
    pub base_domain: String,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::Environment::default().separator("__"))
            .build()?
            .try_deserialize()?;
        Ok(cfg)
    }

    pub fn is_production(&self) -> bool {
        self.env == "production"
    }
}

fn default_host() -> String { "0.0.0.0".into() }
fn default_port() -> u16 { 3000 }
fn default_env() -> String { "development".into() }
fn default_base_domain() -> String { "localhost".into() }

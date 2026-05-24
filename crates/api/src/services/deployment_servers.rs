use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::host_only;

/// Manages deployment containers built as Railpack images.
pub struct DeploymentServers {
    containers: Arc<Mutex<HashMap<Uuid, RunningContainer>>>,
    #[allow(dead_code)]
    work_dir: PathBuf,
    docker_network: String,
    idle_timeout_secs: u64,
    serve_tls: bool,
}

struct RunningContainer {
    name: String,
    last_accessed: Instant,
}

impl DeploymentServers {
    pub fn new(
        work_dir: PathBuf,
        docker_network: String,
        idle_timeout_secs: u64,
        serve_tls: bool,
    ) -> Self {
        Self {
            containers: Arc::new(Mutex::new(HashMap::new())),
            work_dir,
            docker_network,
            idle_timeout_secs,
            serve_tls,
        }
    }

    pub async fn start_image(
        &self,
        deployment_id: Uuid,
        image_ref: &str,
        host: &str,
    ) -> anyhow::Result<()> {
        {
            let mut containers = self.containers.lock().await;
            if let Some(c) = containers.get_mut(&deployment_id) {
                c.last_accessed = Instant::now();
                return Ok(());
            }
        }

        let container_name = format!("serve-{}", deployment_id);
        let _ = tokio::process::Command::new("docker")
            .args(["rm", "-f", &container_name])
            .output()
            .await;

        let router_name = format!("serve-{}", deployment_id.simple());
        let entrypoint = if self.serve_tls { "websecure" } else { "web" };
        let traefik_host = host_only(host);

        let mut args: Vec<String> = vec![
            "run".into(),
            "-d".into(),
            "--name".into(), container_name.clone(),
            "--network".into(), self.docker_network.clone(),
            "--cpus".into(), "0.5".into(),
            "--memory".into(), "512m".into(),
            "--pids-limit".into(), "512".into(),
            "--cap-drop".into(), "ALL".into(),
            "--security-opt".into(), "no-new-privileges".into(),
            "-e".into(), "PORT=3000".into(),
            "-l".into(), "traefik.enable=true".into(),
            "-l".into(), format!("traefik.docker.network={}", self.docker_network),
            "-l".into(), format!("traefik.http.routers.{}.rule=Host(`{}`)", router_name, traefik_host),
            "-l".into(), format!("traefik.http.routers.{}.entrypoints={}", router_name, entrypoint),
            "-l".into(), format!("traefik.http.routers.{}.service={}", router_name, router_name),
        ];

        if self.serve_tls {
            args.extend([
                "-l".into(),
                format!("traefik.http.routers.{}.tls.certresolver=letsencrypt", router_name),
            ]);
        }

        args.extend([
            "-l".into(),
            format!("traefik.http.services.{}.loadbalancer.server.port=3000", router_name),
            image_ref.into(),
        ]);

        let status = tokio::process::Command::new("docker")
            .args(&args)
            .status()
            .await?;

        if !status.success() {
            anyhow::bail!("failed to start deployment container for {}", deployment_id);
        }

        wait_for_container(&container_name, Duration::from_secs(60)).await?;

        let mut containers = self.containers.lock().await;
        containers.insert(
            deployment_id,
            RunningContainer {
                name: container_name.clone(),
                last_accessed: Instant::now(),
            },
        );

        tracing::info!(%deployment_id, container = %container_name, host = %traefik_host, %entrypoint, "started deployment container");
        Ok(())
    }

    pub async fn stop(&self, deployment_id: Uuid) {
        let mut containers = self.containers.lock().await;
        if let Some(c) = containers.remove(&deployment_id) {
            let _ = tokio::process::Command::new("docker")
                .args(["rm", "-f", &c.name])
                .output()
                .await;
            tracing::info!(%deployment_id, container = %c.name, "stopped deployment container");
        }
    }

    pub async fn cleanup_idle(&self) {
        let mut containers = self.containers.lock().await;
        let timeout = Duration::from_secs(self.idle_timeout_secs);
        let now = Instant::now();

        let to_remove: Vec<(Uuid, String)> = containers
            .iter()
            .filter(|(_, c)| now.duration_since(c.last_accessed) > timeout)
            .map(|(id, c)| (*id, c.name.clone()))
            .collect();

        for (id, name) in to_remove {
            containers.remove(&id);
            let _ = tokio::process::Command::new("docker")
                .args(["rm", "-f", &name])
                .output()
                .await;
            tracing::info!(%id, container = %name, "cleaned up idle deployment container");
        }
    }
}

async fn wait_for_container(container_name: &str, timeout: Duration) -> anyhow::Result<()> {
    let url = format!("http://{}:3000/", container_name);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if client.get(&url).send().await.is_ok() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!(
                "deployment container {} did not become ready within timeout",
                container_name
            );
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

/// Manages running Next.js standalone deployment containers.
pub struct DeploymentServers {
    containers: Arc<Mutex<HashMap<Uuid, RunningContainer>>>,
    work_dir: PathBuf,
    docker_network: String,
    idle_timeout_secs: u64,
}

struct RunningContainer {
    name: String,
    last_accessed: Instant,
}

impl DeploymentServers {
    pub fn new(work_dir: PathBuf, docker_network: String, idle_timeout_secs: u64) -> Self {
        Self {
            containers: Arc::new(Mutex::new(HashMap::new())),
            work_dir,
            docker_network,
            idle_timeout_secs,
        }
    }

    /// Return the container's base URL (reachable within the Docker network), starting it if necessary.
    pub async fn get_or_start(
        &self,
        deployment_id: Uuid,
        artifact_key: &str,
        s3_client: &aws_sdk_s3::Client,
        bucket: &str,
    ) -> anyhow::Result<String> {
        {
            let mut containers = self.containers.lock().await;
            if let Some(c) = containers.get_mut(&deployment_id) {
                c.last_accessed = Instant::now();
                return Ok(container_url(&c.name));
            }
        }

        // Prepare files and start the container without holding the lock.
        let deploy_dir = self.work_dir.join(deployment_id.to_string());
        let standalone_dir = deploy_dir.join("standalone");

        if !standalone_dir.join("server.js").exists() {
            tokio::fs::create_dir_all(&deploy_dir).await?;
            download_standalone(s3_client, bucket, artifact_key, &deploy_dir).await?;
        }

        if !standalone_dir.join("server.js").exists() {
            anyhow::bail!(
                "no standalone build found for deployment {} (artifact_key={})",
                deployment_id,
                artifact_key
            );
        }

        let container_name = format!("serve-{}", deployment_id);

        // Remove any stale container from a previous run.
        let _ = tokio::process::Command::new("docker")
            .args(["rm", "-f", &container_name])
            .output()
            .await;

        let standalone_path = standalone_dir
            .canonicalize()
            .unwrap_or(standalone_dir.clone());

        let status = tokio::process::Command::new("docker")
            .args([
                "run",
                "-d",
                "--name",
                &container_name,
                "--network",
                &self.docker_network,
                "-e",
                "NODE_ENV=production",
                "-e",
                "PORT=3000",
                "-e",
                "HOSTNAME=0.0.0.0",
                "-v",
                &format!("{}:/app:ro", standalone_path.display()),
                "--workdir",
                "/app",
                "node:22-alpine",
                "node",
                "server.js",
            ])
            .status()
            .await?;

        if !status.success() {
            anyhow::bail!("failed to start deployment container for {}", deployment_id);
        }

        tracing::info!(%deployment_id, container = %container_name, "started deployment container");

        {
            let mut containers = self.containers.lock().await;
            // Prefer an already-started entry (race between concurrent first requests).
            if let Some(c) = containers.get_mut(&deployment_id) {
                c.last_accessed = Instant::now();
                let _ = tokio::process::Command::new("docker")
                    .args(["rm", "-f", &container_name])
                    .output()
                    .await;
                return Ok(container_url(&c.name));
            }
            containers.insert(
                deployment_id,
                RunningContainer {
                    name: container_name.clone(),
                    last_accessed: Instant::now(),
                },
            );
        }

        wait_for_container(&container_name, Duration::from_secs(30)).await?;
        Ok(container_url(&container_name))
    }

    /// Stop and remove the container for a deployment.
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

    /// Kill containers that haven't served a request within the idle timeout.
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

fn container_url(name: &str) -> String {
    format!("http://{}:3000", name)
}

/// Downloads all objects under `s3_prefix`, stripping `strip_prefix` from
/// each key to produce the local relative path. Paginates until exhausted.
async fn download_prefix(
    s3_client: &aws_sdk_s3::Client,
    bucket: &str,
    s3_prefix: &str,
    strip_prefix: &str,
    local_dir: &PathBuf,
) -> anyhow::Result<u64> {
    let mut count: u64 = 0;
    let mut continuation_token: Option<String> = None;

    loop {
        let mut req = s3_client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(s3_prefix);

        if let Some(ref token) = continuation_token {
            req = req.continuation_token(token);
        }

        let resp = req.send().await?;
        let truncated = resp.is_truncated.unwrap_or(false);
        continuation_token = resp.next_continuation_token.clone();

        for obj in resp.contents.unwrap_or_default() {
            let key = match obj.key() {
                Some(k) if !k.ends_with('/') => k,
                _ => continue,
            };

            let relative = key.strip_prefix(strip_prefix).unwrap_or(key);
            let local_path = local_dir.join(relative.trim_start_matches('/'));

            if let Some(parent) = local_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }

            let bytes = s3_client
                .get_object()
                .bucket(bucket)
                .key(key)
                .send()
                .await?
                .body
                .collect()
                .await?
                .into_bytes();

            tokio::fs::write(&local_path, &bytes).await?;
            count += 1;
        }

        if !truncated {
            break;
        }
    }

    Ok(count)
}

async fn download_standalone(
    s3_client: &aws_sdk_s3::Client,
    bucket: &str,
    artifact_key: &str,
    deploy_dir: &PathBuf,
) -> anyhow::Result<()> {
    let prefix = artifact_key.trim_end_matches('/');

    let standalone_prefix = format!("{}/standalone/", prefix);
    let n = download_prefix(
        s3_client,
        bucket,
        &standalone_prefix,
        &standalone_prefix,
        &deploy_dir.join("standalone"),
    )
    .await?;
    tracing::info!(%n, "downloaded standalone files");
    tracing::info!(?deploy_dir, "downloaded standalone build");
    Ok(())
}

/// Poll the container's HTTP endpoint (via Docker network name) until it responds.
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

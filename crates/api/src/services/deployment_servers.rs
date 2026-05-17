use std::collections::HashMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Child;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Manages running Next.js standalone server processes per deployment.
pub struct DeploymentServers {
    servers: Arc<Mutex<HashMap<Uuid, RunningServer>>>,
    work_dir: PathBuf,
    idle_timeout_secs: u64,
}

struct RunningServer {
    port: u16,
    child: Child,
    last_accessed: Instant,
}

impl DeploymentServers {
    pub fn new(work_dir: PathBuf, idle_timeout_secs: u64) -> Self {
        Self {
            servers: Arc::new(Mutex::new(HashMap::new())),
            work_dir,
            idle_timeout_secs,
        }
    }

    /// Get or start a server for the given deployment.
    /// Returns the port the server is listening on.
    pub async fn get_or_start(
        &self,
        deployment_id: Uuid,
        artifact_key: &str,
        s3_client: &aws_sdk_s3::Client,
        bucket: &str,
    ) -> anyhow::Result<u16> {
        {
            let mut servers = self.servers.lock().await;
            if let Some(server) = servers.get_mut(&deployment_id) {
                server.last_accessed = Instant::now();
                return Ok(server.port);
            }
        }

        // Download and start without holding the lock so other requests aren't blocked.
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

        let port = find_free_port()?;
        let mut cmd = tokio::process::Command::new("node");
        cmd.args(["server.js"])
            .current_dir(&standalone_dir)
            .env("NODE_ENV", "production")
            .env("PORT", port.to_string())
            .env("HOSTNAME", "127.0.0.1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);

        let child = cmd.spawn()?;
        tracing::info!(%deployment_id, %port, "started next.js standalone server");

        {
            let mut servers = self.servers.lock().await;
            // Another request may have started it while we were downloading; prefer existing.
            if let Some(server) = servers.get_mut(&deployment_id) {
                server.last_accessed = Instant::now();
                return Ok(server.port);
            }
            servers.insert(
                deployment_id,
                RunningServer {
                    port,
                    child,
                    last_accessed: Instant::now(),
                },
            );
        }

        // Wait for the server to accept connections (up to 10 s).
        wait_for_port(port, Duration::from_secs(10)).await?;

        Ok(port)
    }

    /// Remove and stop the server for a deployment.
    pub async fn stop(&self, deployment_id: Uuid) {
        let mut servers = self.servers.lock().await;
        if let Some(mut server) = servers.remove(&deployment_id) {
            let _ = server.child.start_kill();
            tracing::info!(%deployment_id, "stopped next.js standalone server");
        }
    }

    /// Cleanup idle servers.
    pub async fn cleanup_idle(&self) {
        let mut servers = self.servers.lock().await;
        let timeout = Duration::from_secs(self.idle_timeout_secs);
        let now = Instant::now();

        let to_remove: Vec<Uuid> = servers
            .iter()
            .filter(|(_, s)| now.duration_since(s.last_accessed) > timeout)
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            if let Some(mut server) = servers.remove(&id) {
                let _ = server.child.start_kill();
                tracing::info!(%id, "cleaned up idle next.js server");
            }
        }
    }
}

/// Downloads the standalone build from MinIO to local disk.
async fn download_standalone(
    s3_client: &aws_sdk_s3::Client,
    bucket: &str,
    artifact_key: &str,
    deploy_dir: &PathBuf,
) -> anyhow::Result<()> {
    let prefix = artifact_key.trim_end_matches('/');
    let standalone_prefix = format!("{}/standalone", prefix);

    let list_resp = s3_client
        .list_objects_v2()
        .bucket(bucket)
        .prefix(&standalone_prefix)
        .send()
        .await?;

    if let Some(contents) = list_resp.contents {
        for obj in contents {
            if let Some(key) = obj.key() {
                let relative = key.strip_prefix(&standalone_prefix).unwrap_or(key);
                let local_path = deploy_dir.join("standalone").join(relative.trim_start_matches('/'));

                if let Some(parent) = local_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }

                let get_resp = s3_client
                    .get_object()
                    .bucket(bucket)
                    .key(key)
                    .send()
                    .await?;

                let bytes = get_resp.body.collect().await?.into_bytes();
                tokio::fs::write(&local_path, &bytes).await?;
            }
        }
    }

    // Also download .next/static if it exists (for static assets referenced by standalone)
    let static_prefix = format!("{}/.next/static", prefix);
    let static_strip = format!("{}/.next/", prefix);
    if let Ok(list_resp) = s3_client
        .list_objects_v2()
        .bucket(bucket)
        .prefix(&static_prefix)
        .send()
        .await
    {
        if let Some(contents) = list_resp.contents {
            for obj in contents {
                if let Some(key) = obj.key() {
                    // Strip "{uuid}/.next/" so we get "static/..." and place under standalone/.next/
                    let relative = key.strip_prefix(&static_strip).unwrap_or(key);
                    let local_path = deploy_dir.join("standalone").join(".next").join(relative.trim_start_matches('/'));

                    if let Some(parent) = local_path.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }

                    let get_resp = s3_client
                        .get_object()
                        .bucket(bucket)
                        .key(key)
                        .send()
                        .await?;

                    let bytes = get_resp.body.collect().await?.into_bytes();
                    tokio::fs::write(&local_path, &bytes).await?;
                }
            }
        }
    }

    tracing::info!(?deploy_dir, "downloaded standalone build");
    Ok(())
}

fn find_free_port() -> anyhow::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    Ok(port)
}

async fn wait_for_port(port: u16, timeout: Duration) -> anyhow::Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        if tokio::net::TcpStream::connect(&addr).await.is_ok() {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("next.js server on port {} did not start within timeout", port);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

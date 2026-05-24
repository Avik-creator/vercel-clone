use std::future::Future;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;

use crate::models::{BuildJob, LogLine};
use crate::nats::WorkerNats;

pub async fn run_build(
    job: &BuildJob,
    nats: &WorkerNats,
    work_dir: &Path,
    registry_url: &str,
    build_registry_url: &str,
    _build_network: &str,
    build_timeout: Duration,
) -> anyhow::Result<String> {
    clone_repo(job, work_dir, nats).await?;

    // BuildKit pushes directly to the registry using its Docker-network hostname.
    // This avoids sending the image tarball back to the client (which hangs on large images).
    let build_image_ref = image_tag(build_registry_url, job.deployment_id);

    // The image_ref stored in the DB uses the host-accessible hostname so Docker can pull it.
    let serve_image_ref = image_tag(registry_url, job.deployment_id);

    // --output image tells BuildKit to push directly to the registry (type=image,push=true).
    // No tarball is transferred back to the worker client.
    let mut cmd = Command::new("nixpacks");
    cmd.args(["build", "--name", &build_image_ref, "-o", "image", "."]);
    cmd.current_dir(work_dir);
    cmd.env("BUILDKIT_HOST", "unix:///var/run/buildkit/buildkitd.sock");
    run_logged_command(
        "nixpacks build",
        &mut cmd,
        job.deployment_id,
        nats,
        build_timeout,
    )
    .await?;

    Ok(serve_image_ref)
}

fn image_tag(registry_url: &str, deployment_id: uuid::Uuid) -> String {
    format!(
        "{}/deployment-{}:latest",
        registry_url.trim_end_matches('/'),
        deployment_id
    )
}

async fn clone_repo(job: &BuildJob, work_dir: &Path, nats: &WorkerNats) -> anyhow::Result<()> {
    let git_url = if let Some(ref token) = job.github_token {
        job.git_url.replace(
            "https://github.com/",
            &format!("https://x-access-token:{}@github.com/", token),
        )
    } else {
        job.git_url.clone()
    };

    let _ = nats
        .publish_log(&LogLine {
            deployment_id: job.deployment_id,
            line: "cloning repository".to_string(),
            timestamp: chrono::Utc::now(),
        })
        .await;

    let status = Command::new("git")
        .args(["clone", &git_url, "."])
        .current_dir(work_dir)
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("git clone failed with exit code {}", status.code().unwrap_or(-1));
    }

    let status = Command::new("git")
        .args(["checkout", &job.commit_sha])
        .current_dir(work_dir)
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("git checkout failed with exit code {}", status.code().unwrap_or(-1));
    }

    Ok(())
}

async fn run_logged_command(
    name: &str,
    cmd: &mut Command,
    deployment_id: uuid::Uuid,
    nats: &WorkerNats,
    timeout: Duration,
) -> anyhow::Result<()> {
    let _ = nats
        .publish_log(&LogLine {
            deployment_id,
            line: format!("running {}", name),
            timestamp: chrono::Utc::now(),
        })
        .await;

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.kill_on_drop(true);
    let mut child = cmd.spawn()?;
    let stdout = take_child_pipe(child.stdout.take(), "stdout")?;
    let stderr = take_child_pipe(child.stderr.take(), "stderr")?;

    let stdout_task = spawn_log_pipe(stdout, deployment_id, nats.clone(), "stdout");
    let stderr_task = spawn_log_pipe(stderr, deployment_id, nats.clone(), "stderr");

    let status = wait_for_status_with_timeout(
        async {
            let status = child.wait().await;
            let _ = tokio::join!(stdout_task, stderr_task);
            status
        },
        timeout,
    )
    .await?;

    if !status.success() {
        anyhow::bail!("{} failed with exit code {}", name, status.code().unwrap_or(-1));
    }

    Ok(())
}

fn spawn_log_pipe<T>(
    pipe: T,
    deployment_id: uuid::Uuid,
    nats: WorkerNats,
    stream_name: &'static str,
) -> tokio::task::JoinHandle<()>
where
    T: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(pipe).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = nats
                .publish_log(&LogLine {
                    deployment_id,
                    line: format!("[{}] {}", stream_name, line),
                    timestamp: chrono::Utc::now(),
                })
                .await;
        }
    })
}

fn take_child_pipe<T>(pipe: Option<T>, name: &str) -> anyhow::Result<T>
where
    T: AsyncRead + Unpin,
{
    pipe.ok_or_else(|| anyhow::anyhow!("command child missing {} pipe", name))
}

async fn wait_for_status_with_timeout<F>(
    wait: F,
    build_timeout: Duration,
) -> anyhow::Result<std::process::ExitStatus>
where
    F: Future<Output = std::io::Result<std::process::ExitStatus>>,
{
    tokio::time::timeout(build_timeout, wait)
        .await
        .map_err(|_| anyhow::anyhow!("nixpacks build timed out after {} seconds", build_timeout.as_secs()))?
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_child_pipe_returns_error_instead_of_panicking() {
        let result = take_child_pipe(Option::<tokio::io::Empty>::None, "stdout");

        assert!(matches!(result, Err(err) if err.to_string().contains("stdout")));
    }

    #[tokio::test]
    async fn build_timeout_is_applied_to_container_wait() {
        let wait = async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Command::new("true").status().await
        };

        let build = wait_for_status_with_timeout(wait, std::time::Duration::from_millis(1)).await;
        assert!(matches!(build, Err(err) if err.to_string().contains("timed out")));
    }

    #[test]
    fn image_tag_targets_local_registry() {
        let deployment_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

        assert_eq!(
            image_tag("localhost:5000", deployment_id),
            "localhost:5000/deployment-00000000-0000-0000-0000-000000000001:latest"
        );
    }
}

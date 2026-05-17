use std::future::Future;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;

use crate::models::{BuildJob, LogLine};
use crate::nats::WorkerNats;

pub async fn run_build(
    job: &BuildJob,
    nats: &WorkerNats,
    docker_network: &str,
    build_timeout: Duration,
) -> anyhow::Result<()> {
    let container_name = format!("build-{}", job.deployment_id);

    let node_image = detect_runtime(job).await;
    let is_node_runtime = node_image.starts_with("node:");

    let git_url = if let Some(ref token) = job.github_token {
        job.git_url.replace(
            "https://github.com/",
            &format!("https://x-access-token:{}@github.com/", token),
        )
    } else {
        job.git_url.clone()
    };

    let build_cmd = if let Some(ref cmd) = job.build_command {
        cmd.clone()
    } else {
        "npm run build".to_string()
    };

    // Install git (and common native build deps) in both Alpine and Debian-based images.
    // We suppress most output to keep logs readable; failures still surface with a non-zero exit.
    let deps_cmd = r#"if command -v apk >/dev/null 2>&1; then \
    apk add --no-cache git python3 make g++ ca-certificates > /dev/null; \
elif command -v apt-get >/dev/null 2>&1; then \
    apt-get update -y > /dev/null && apt-get install -y git python3 make g++ ca-certificates > /dev/null; \
else \
    echo 'No supported package manager (apk/apt-get) found' 1>&2; \
    exit 127; \
fi"#;

    // For Node projects, install dependencies before running the build command.
    // - Uses pnpm/yarn when lockfiles are present (via corepack), otherwise npm.
    let node_install_cmd = r#"corepack enable > /dev/null 2>&1 || true; \
if [ -f pnpm-lock.yaml ]; then \
    corepack prepare pnpm@latest --activate > /dev/null 2>&1 || true; \
    pnpm install --frozen-lockfile; \
elif [ -f yarn.lock ]; then \
    corepack prepare yarn@stable --activate > /dev/null 2>&1 || true; \
    yarn install --frozen-lockfile; \
elif [ -f package-lock.json ]; then \
    npm ci; \
else \
    npm install; \
fi"#;

    let full_cmd = if is_node_runtime {
        format!(
            "set -e; {} && git clone {} /app/repo && cd /app/repo && git checkout {} && {} && {}",
            deps_cmd, git_url, job.commit_sha, node_install_cmd, build_cmd
        )
    } else {
        // Non-Node runtimes (e.g. Rust) should provide their own build command.
        format!(
            "set -e; {} && git clone {} /app/repo && cd /app/repo && git checkout {} && {}",
            deps_cmd, git_url, job.commit_sha, build_cmd
        )
    };

    let mut cmd = Command::new("docker");
    cmd.args([
        "run",
        "--name",
        &container_name,
        "--network",
        docker_network,
        "--workdir",
        "/app",
        "-e",
        &format!("PROJECT_ID={}", job.project_id),
        "-e",
        &format!("DEPLOYMENT_ID={}", job.deployment_id),
    ]);

    for (key, value) in &job.env_vars {
        cmd.arg("-e").arg(format!("{}={}", key, value));
    }

    cmd.args([&node_image, "sh", "-c", &full_cmd]);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.kill_on_drop(true);

    let _ = nats
        .publish_log(&LogLine {
            deployment_id: job.deployment_id,
            line: format!("running build in {}", node_image),
            timestamp: chrono::Utc::now(),
        })
        .await;

    let mut child = cmd.spawn()?;

    let stdout = take_child_pipe(child.stdout.take(), "stdout")?;
    let stderr = take_child_pipe(child.stderr.take(), "stderr")?;

    let deployment_id = job.deployment_id;
    let nats_clone = nats.clone();

    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let log = LogLine {
                deployment_id,
                line: format!("[stdout] {}", line),
                timestamp: chrono::Utc::now(),
            };
            let _ = nats_clone.publish_log(&log).await;
        }
    });

    let deployment_id = job.deployment_id;
    let nats_clone = nats.clone();

    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let log = LogLine {
                deployment_id,
                line: format!("[stderr] {}", line),
                timestamp: chrono::Utc::now(),
            };
            let _ = nats_clone.publish_log(&log).await;
        }
    });

    let status = wait_for_status_with_timeout(
        async {
            let _ = tokio::join!(stdout_task, stderr_task);
            child.wait().await
        },
        build_timeout,
    )
    .await?;

    if !status.success() {
        anyhow::bail!("build failed with exit code: {:?}", status.code());
    }

    Ok(())
}

fn take_child_pipe<T>(pipe: Option<T>, name: &str) -> anyhow::Result<T>
where
    T: AsyncRead + Unpin,
{
    pipe.ok_or_else(|| anyhow::anyhow!("docker child missing {} pipe", name))
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
        .map_err(|_| anyhow::anyhow!("build timed out after {} seconds", build_timeout.as_secs()))?
        .map_err(Into::into)
}

async fn detect_runtime(job: &BuildJob) -> &'static str {
    if job
        .build_command
        .as_ref()
        .map_or(false, |c| c.contains("cargo"))
    {
        "rust:slim"
    } else {
        "node:22-alpine"
    }
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
}

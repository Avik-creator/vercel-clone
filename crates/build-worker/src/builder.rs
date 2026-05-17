use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::models::{BuildJob, LogLine};
use crate::nats::WorkerNats;

pub async fn run_build(
    job: &BuildJob,
    nats: &WorkerNats,
    docker_network: &str,
) -> anyhow::Result<()> {
    let container_name = format!("build-{}", job.deployment_id);

    let node_image = detect_runtime(job).await;

    let git_url = if let Some(ref token) = job.github_token {
        job.git_url.replace("https://github.com/", &format!("https://x-access-token:{}@github.com/", token))
    } else {
        job.git_url.clone()
    };

    let build_cmd = if let Some(ref cmd) = job.build_command {
        cmd.clone()
    } else {
        "npm run build".to_string()
    };

    let full_cmd = format!(
        "apk add --no-cache git > /dev/null 2>&1 && git clone {} /app/repo && cd /app/repo && git checkout {} && {}",
        git_url, job.commit_sha, build_cmd
    );

    let mut cmd = Command::new("docker");
    cmd.args([
        "run",
        "--name", &container_name,
        "--network", docker_network,
        "--workdir", "/app",
        "-e", &format!("PROJECT_ID={}", job.project_id),
        "-e", &format!("DEPLOYMENT_ID={}", job.deployment_id),
        &node_image,
        "sh", "-c", &full_cmd,
    ]);

    for (key, value) in &job.env_vars {
        cmd.arg("-e").arg(format!("{}={}", key, value));
    }

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().expect("stdout");
    let stderr = child.stderr.take().expect("stderr");

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

    let _ = tokio::join!(stdout_task, stderr_task);

    let status = child.wait().await?;

    if !status.success() {
        anyhow::bail!("build failed with exit code: {:?}", status.code());
    }

    Ok(())
}

async fn detect_runtime(job: &BuildJob) -> &'static str {
    if job.build_command.as_ref().map_or(false, |c| c.contains("cargo")) {
        "rust:slim"
    } else {
        "node:22-alpine"
    }
}

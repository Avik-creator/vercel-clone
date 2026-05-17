use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::models::LogLine;
use crate::nats::WorkerNats;

pub async fn run_build(
    job: &crate::models::BuildJob,
    work_dir: &Path,
    nats: &WorkerNats,
    docker_network: &str,
) -> anyhow::Result<()> {
    let container_name = format!("build-{}", job.deployment_id);

    let repo_path = work_dir.join("repo").canonicalize()?;
    let repo_path_str = repo_path.to_string_lossy();

    let node_image = detect_runtime(&repo_path).await;

    let mut cmd = Command::new("docker");
    cmd.args([
        "run", "--rm",
        "--name", &container_name,
        "--network", docker_network,
        "--workdir", "/app",
        "-v", &format!("{}:/app", repo_path_str),
        &node_image,
    ]);

    if let Some(ref build_cmd) = job.build_command {
        cmd.args(["sh", "-c", build_cmd]);
    } else {
        cmd.args(["sh", "-c", "npm run build 2>/dev/null || cargo build --release 2>/dev/null || echo 'no build command found'"]);
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

async fn detect_runtime(repo_path: &Path) -> &'static str {
    let package_json = repo_path.join("package.json");
    let cargo_toml = repo_path.join("Cargo.toml");

    if package_json.exists() {
        "node:22-alpine"
    } else if cargo_toml.exists() {
        "rust:slim"
    } else {
        "node:22-alpine"
    }
}

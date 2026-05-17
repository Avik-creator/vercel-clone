mod builder;
mod config;
mod models;
mod nats;
mod storage;

use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;

use crate::models::{BuildResult, DeploymentState, LogLine};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let env_filter =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info,vercel_clone_worker=debug".to_string());

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .compact()
        .init();

    let config = config::WorkerConfig::load()?;
    tracing::info!("build worker starting");

    let nats = nats::WorkerNats::connect(&config.nats_url).await?;
    tracing::info!(url = %config.nats_url, "nats connected");

    let storage = storage::Storage::new(
        &config.minio_endpoint,
        &config.minio_access_key,
        &config.minio_secret_key,
        &config.minio_bucket,
    )
    .await?;
    tracing::info!(endpoint = %config.minio_endpoint, bucket = %config.minio_bucket, "minio connected");

    let work_base = PathBuf::from("/tmp/builds");
    tokio::fs::create_dir_all(&work_base).await?;

    let semaphore = Arc::new(tokio::sync::Semaphore::new(config.max_concurrent_builds));
    tracing::info!(max_concurrent = config.max_concurrent_builds, "subscribing to build jobs");

    let jobs = nats.subscribe_jobs().await?;
    tokio::pin!(jobs);

    while let Some(job) = jobs.next().await {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let nats = nats.clone();
        let storage = storage.clone();
        let work_base = work_base.clone();
        let docker_network = config.docker_network.clone();
        let build_timeout_secs = config.build_timeout_secs;

        tokio::spawn(async move {
            let _permit = permit;
            let deployment_id = job.deployment_id;
            tracing::info!(
                %deployment_id,
                branch = %job.branch,
                commit = %job.commit_sha,
                "processing build job"
            );

            let _ = nats
                .publish_log(&LogLine {
                    deployment_id,
                    line: "build started".to_string(),
                    timestamp: chrono::Utc::now(),
                })
                .await;

            // Signal that the build is actively running so the API sets build_started_at.
            let _ = nats
                .publish_result(&BuildResult {
                    deployment_id,
                    state: DeploymentState::Building,
                    artifact_key: None,
                    log_output: None,
                    error_message: None,
                })
                .await;

            let result = match process_job(
                &job,
                &nats,
                &storage,
                &work_base,
                &docker_network,
                build_timeout_secs,
            )
            .await
            {
                Ok(artifact_key) => {
                    tracing::info!(%deployment_id, "build succeeded");
                    BuildResult {
                        deployment_id,
                        state: DeploymentState::Ready,
                        artifact_key: Some(artifact_key),
                        log_output: None,
                        error_message: None,
                    }
                }
                Err(e) => {
                    tracing::error!(%deployment_id, error = %e, "build failed");
                    let _ = nats
                        .publish_log(&LogLine {
                            deployment_id,
                            line: format!("error: {}", e),
                            timestamp: chrono::Utc::now(),
                        })
                        .await;
                    BuildResult {
                        deployment_id,
                        state: DeploymentState::Error,
                        artifact_key: None,
                        log_output: None,
                        error_message: Some(e.to_string()),
                    }
                }
            };

            if let Err(e) = nats.publish_result(&result).await {
                tracing::error!(%deployment_id, error = %e, "failed to publish build result");
            }

            let work_dir = work_base.join(deployment_id.to_string());
            let _ = tokio::fs::remove_dir_all(&work_dir).await;

            let container_name = format!("build-{}", deployment_id);
            let _ = tokio::process::Command::new("docker")
                .args(["rm", "-f", &container_name])
                .output()
                .await;
        });
    }

    Ok(())
}

async fn process_job(
    job: &models::BuildJob,
    nats: &nats::WorkerNats,
    storage: &storage::Storage,
    work_base: &std::path::Path,
    docker_network: &str,
    build_timeout_secs: u64,
) -> anyhow::Result<String> {
    let work_dir = work_base.join(job.deployment_id.to_string());
    tokio::fs::create_dir_all(&work_dir).await?;

    let container_name = format!("build-{}", job.deployment_id);

    builder::run_build(
        job,
        nats,
        docker_network,
        std::time::Duration::from_secs(build_timeout_secs),
    )
    .await?;

    let _ = nats
        .publish_log(&LogLine {
            deployment_id: job.deployment_id,
            line: "build completed, extracting artifacts".to_string(),
            timestamp: chrono::Utc::now(),
        })
        .await;

    // Detect what the build produced inside the container.
    let output_type = detect_output_type(&container_name, job.output_dir.as_deref()).await?;

    tracing::info!(
        deployment_id = %job.deployment_id,
        output_type = %output_type.label(),
        "detected build output type"
    );

    let _ = nats
        .publish_log(&LogLine {
            deployment_id: job.deployment_id,
            line: format!("detected output: {}", output_type.label()),
            timestamp: chrono::Utc::now(),
        })
        .await;

    let artifact_key = match output_type {
        OutputType::Standalone => {
            // Next.js standalone deployment: copy standalone/ then put static assets and
            // public/ inside it so server.js serves them without a separate CDN step.
            let local_standalone = work_dir.join("standalone");
            tokio::fs::create_dir_all(&local_standalone).await?;

            docker_cp(&container_name, "/app/repo/.next/standalone/.", &local_standalone).await?;

            // .next/static/ must live at standalone/.next/static/ for server.js to serve it.
            docker_cp(
                &container_name,
                "/app/repo/.next/static/.",
                &local_standalone.join(".next").join("static"),
            )
            .await?;

            // public/ is optional
            let public_cp = tokio::process::Command::new("docker")
                .args([
                    "cp",
                    &format!("{}:/app/repo/public/.", &container_name),
                    local_standalone.join("public").to_str().unwrap(),
                ])
                .status()
                .await;
            if let Ok(s) = public_cp {
                if !s.success() {
                    let _ = tokio::fs::remove_dir_all(local_standalone.join("public")).await;
                }
            }

            storage
                .upload_dir_with_prefix(job.deployment_id, &local_standalone, "standalone", nats)
                .await?
        }
        OutputType::Static(ref dir) => {
            let local_output = work_dir.join("output");
            tokio::fs::create_dir_all(&local_output).await?;
            docker_cp(
                &container_name,
                &format!("/app/repo/{}/.", dir),
                &local_output,
            )
            .await?;
            storage
                .upload_dir(job.deployment_id, &local_output, nats)
                .await?
        }
    };

    let _ = nats
        .publish_log(&LogLine {
            deployment_id: job.deployment_id,
            line: format!("artifacts uploaded to {}", artifact_key),
            timestamp: chrono::Utc::now(),
        })
        .await;

    Ok(artifact_key)
}

enum OutputType {
    Standalone,
    Static(String),
}

impl OutputType {
    fn label(&self) -> &str {
        match self {
            OutputType::Standalone => "next.js standalone",
            OutputType::Static(_) => "static",
        }
    }
}

async fn detect_output_type(
    container_name: &str,
    configured_output_dir: Option<&str>,
) -> anyhow::Result<OutputType> {
    // docker cp works on stopped containers; use it to probe for files.
    // `docker cp {container}:{path} -` exits 0 if the path exists, non-zero otherwise.
    let standalone_exists = tokio::process::Command::new("docker")
        .args([
            "cp",
            &format!("{}:/app/repo/.next/standalone/server.js", container_name),
            "-",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await?
        .success();

    if standalone_exists {
        return Ok(OutputType::Standalone);
    }

    // Use configured output_dir if provided, otherwise auto-detect.
    if let Some(dir) = configured_output_dir {
        return Ok(OutputType::Static(dir.to_string()));
    }

    // Auto-detect: try common static output dirs in priority order.
    for dir in &["out", "build", "dist", ".next"] {
        let exists = tokio::process::Command::new("docker")
            .args([
                "cp",
                &format!("{}:/app/repo/{}/.", container_name, dir),
                "-",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);

        if exists {
            return Ok(OutputType::Static(dir.to_string()));
        }
    }

    Ok(OutputType::Static("dist".to_string()))
}

async fn docker_cp(container: &str, src: &str, dest: &std::path::Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(dest).await?;
    let status = tokio::process::Command::new("docker")
        .args(["cp", &format!("{}:{}", container, src), dest.to_str().unwrap()])
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("docker cp {}:{} failed", container, src);
    }
    Ok(())
}

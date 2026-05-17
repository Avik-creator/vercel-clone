mod builder;
mod config;
mod models;
mod nats;
mod storage;

use futures::StreamExt;
use std::path::PathBuf;

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

    tracing::info!("subscribing to build jobs");

    let jobs = nats.subscribe_jobs().await?;
    tokio::pin!(jobs);

    while let Some(job) = jobs.next().await {
        let nats = nats.clone();
        let storage = storage.clone();
        let work_base = work_base.clone();
        let docker_network = config.docker_network.clone();
        let build_timeout_secs = config.build_timeout_secs;

        tokio::spawn(async move {
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

    let output_dir = job.output_dir.as_deref().unwrap_or("dist").to_string();
    let container_name = format!("build-{}", job.deployment_id);

    builder::run_build(
        job,
        nats,
        docker_network,
        std::time::Duration::from_secs(build_timeout_secs),
    )
    .await?;

    let log = LogLine {
        deployment_id: job.deployment_id,
        line: "build completed, extracting artifacts".to_string(),
        timestamp: chrono::Utc::now(),
    };
    let _ = nats.publish_log(&log).await;

    // Copy build output from container to host
    let local_output = work_dir.join("output");
    tokio::fs::create_dir_all(&local_output).await?;

    // Append /. so docker cp copies the *contents* of output_dir into local_output,
    // not the directory itself (which would create an extra nesting level).
    let status = tokio::process::Command::new("docker")
        .args([
            "cp",
            &format!("{}:/app/repo/{}/.", container_name, output_dir),
            local_output.to_str().unwrap(),
        ])
        .status()
        .await?;

    if !status.success() {
        anyhow::bail!("failed to extract build output from container");
    }

    let artifact_key = storage
        .upload_dir(job.deployment_id, &local_output, nats)
        .await?;

    let log = LogLine {
        deployment_id: job.deployment_id,
        line: format!("artifacts uploaded to {}", artifact_key),
        timestamp: chrono::Utc::now(),
    };
    let _ = nats.publish_log(&log).await;

    Ok(artifact_key)
}

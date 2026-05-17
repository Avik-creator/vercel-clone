mod config;
mod git;
mod builder;
mod models;
mod nats;
mod storage;

use std::path::PathBuf;
use futures::StreamExt;

use crate::models::{BuildResult, DeploymentState, LogLine};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let env_filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "info,vercel_clone_worker=debug".to_string());

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

    let mut jobs = nats.subscribe_jobs().await?;
    tokio::pin!(jobs);

    while let Some(job) = jobs.next().await {
        let deployment_id = job.deployment_id;
        tracing::info!(
            %deployment_id,
            branch = %job.branch,
            commit = %job.commit_sha,
            "processing build job"
        );

        let log = LogLine {
            deployment_id,
            line: "build started".to_string(),
            timestamp: chrono::Utc::now(),
        };
        let _ = nats.publish_log(&log).await;

        let result = match process_job(&job, &nats, &storage, &work_base, &config.docker_network).await {
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
                let _ = nats.publish_log(&LogLine {
                    deployment_id,
                    line: format!("error: {}", e),
                    timestamp: chrono::Utc::now(),
                }).await;
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
    }

    Ok(())
}

async fn process_job(
    job: &models::BuildJob,
    nats: &nats::WorkerNats,
    storage: &storage::Storage,
    work_base: &std::path::Path,
    docker_network: &str,
) -> anyhow::Result<String> {
    let work_dir = work_base.join(job.deployment_id.to_string());
    tokio::fs::create_dir_all(&work_dir).await?;

    let log = LogLine {
        deployment_id: job.deployment_id,
        line: "cloning repository".to_string(),
        timestamp: chrono::Utc::now(),
    };
    let _ = nats.publish_log(&log).await;

    git::clone_repo(job, &work_dir).await?;

    let log = LogLine {
        deployment_id: job.deployment_id,
        line: "repository cloned, starting build".to_string(),
        timestamp: chrono::Utc::now(),
    };
    let _ = nats.publish_log(&log).await;

    builder::run_build(job, &work_dir, nats, docker_network).await?;

    let log = LogLine {
        deployment_id: job.deployment_id,
        line: "build completed, uploading artifacts".to_string(),
        timestamp: chrono::Utc::now(),
    };
    let _ = nats.publish_log(&log).await;

    let output_dir = job.output_dir.as_deref().unwrap_or("dist");
    let artifact_path = work_dir.join("repo").join(output_dir);

    if !artifact_path.exists() {
        let log = LogLine {
            deployment_id: job.deployment_id,
            line: format!("warning: output directory '{}' not found, uploading entire repo", output_dir),
            timestamp: chrono::Utc::now(),
        };
        let _ = nats.publish_log(&log).await;
    }

    let upload_path = if artifact_path.exists() {
        artifact_path
    } else {
        work_dir.join("repo")
    };

    let artifact_key = storage.upload_dir(job.deployment_id, &upload_path, nats).await?;

    let log = LogLine {
        deployment_id: job.deployment_id,
        line: format!("artifacts uploaded to {}", artifact_key),
        timestamp: chrono::Utc::now(),
    };
    let _ = nats.publish_log(&log).await;

    Ok(artifact_key)
}

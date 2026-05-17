mod builder;
mod config;
mod models;
mod nats;

use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;

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

    let nats = nats::WorkerNats::connect(
        &config.nats_url,
        config.nats_user.as_deref(),
        config.nats_password.as_deref(),
    ).await?;
    tracing::info!(url = %config.nats_url, "nats connected");

    let work_base = PathBuf::from("/tmp/builds");
    tokio::fs::create_dir_all(&work_base).await?;

    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            let _ = tokio::fs::write("/tmp/worker-alive", "").await;
        }
    });

    let semaphore = Arc::new(tokio::sync::Semaphore::new(config.max_concurrent_builds));
    tracing::info!(max_concurrent = config.max_concurrent_builds, "subscribing to build jobs");

    let jobs = nats.subscribe_jobs().await?;
    tokio::pin!(jobs);

    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown => {
                tracing::info!("shutdown signal received, draining in-flight builds...");
                semaphore.close();
                break;
            }
            job = jobs.next() => {
                let Some(job) = job else {
                    tracing::info!("job stream ended");
                    break;
                };

                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => {
                        tracing::info!("semaphore closed, rejecting job");
                        continue;
                    }
                };

                let nats = nats.clone();
                let work_base = work_base.clone();
                let registry_url = config.registry_url.clone();
                let build_registry_url = config.build_registry_url
                    .clone()
                    .unwrap_or_else(|| config.registry_url.clone());
                let build_network = config.build_network.clone();
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

                    let _ = nats
                        .publish_result(&BuildResult {
                            deployment_id,
                            state: DeploymentState::Building,
                            artifact_key: None,
                            image_ref: None,
                            log_output: None,
                            error_message: None,
                        })
                        .await;

                    let result = match process_job(
                        &job,
                        &nats,
                        &work_base,
                        &registry_url,
                        &build_registry_url,
                        &build_network,
                        build_timeout_secs,
                    )
                    .await
                    {
                        Ok(image_ref) => {
                            tracing::info!(%deployment_id, "build succeeded");
                            BuildResult {
                                deployment_id,
                                state: DeploymentState::Ready,
                                artifact_key: None,
                                image_ref: Some(image_ref),
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
                                image_ref: None,
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

                });
            }
        }
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _ = tokio::fs::remove_file("/tmp/worker-alive").await;
    tracing::info!("worker shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn process_job(
    job: &models::BuildJob,
    nats: &nats::WorkerNats,
    work_base: &std::path::Path,
    registry_url: &str,
    build_registry_url: &str,
    build_network: &str,
    build_timeout_secs: u64,
) -> anyhow::Result<String> {
    let work_dir = work_base.join(job.deployment_id.to_string());
    tokio::fs::create_dir_all(&work_dir).await?;

    let image_ref = builder::run_build(
        job,
        nats,
        &work_dir,
        registry_url,
        build_registry_url,
        build_network,
        std::time::Duration::from_secs(build_timeout_secs),
    )
    .await?;

    let _ = nats
        .publish_log(&LogLine {
            deployment_id: job.deployment_id,
            line: format!("image pushed: {}", image_ref),
            timestamp: chrono::Utc::now(),
        })
        .await;

    Ok(image_ref)
}

use async_nats::jetstream::stream::Stream;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    config::AppConfig,
    errors::AppError,
    model::build_job::{BuildJob, BuildResult, LogLine},
};

pub const JOBS_SUBJECT: &str = "build.jobs.>";
pub const RESULTS_SUBJECT: &str = "build.results.>";
pub const LOGS_SUBJECT: &str = "build.logs.>";

#[derive(Clone)]
pub struct NatsClient {
    pub client: async_nats::Client,
    pub context: async_nats::jetstream::Context,
    pub log_broadcasts: Arc<tokio::sync::Mutex<HashMap<Uuid, broadcast::Sender<LogLine>>>>,
}

impl NatsClient {
    pub async fn connect(config: &AppConfig) -> Result<Self, AppError> {
        let client = async_nats::connect(&config.nats_url)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to connect to NATS: {}", e)))?;

        let context = async_nats::jetstream::new(client.clone());

        ensure_stream(&context, "build_jobs", vec!["build.jobs.>", "build.jobs"]).await?;
        ensure_stream(
            &context,
            "build_results",
            vec!["build.results.>", "build.results"],
        )
        .await?;
        ensure_stream(&context, "build_logs", vec!["build.logs.>", "build.logs"]).await?;

        Ok(Self {
            client,
            context,
            log_broadcasts: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        })
    }

    pub async fn publish_job(&self, job: &BuildJob) -> Result<(), AppError> {
        let subject = format!("build.jobs.{}", job.deployment_id);
        publish(&self.client, &subject, job).await
    }

    pub async fn publish_result(&self, result: &BuildResult) -> Result<(), AppError> {
        let subject = format!("build.results.{}", result.deployment_id);
        publish(&self.client, &subject, result).await
    }

    pub async fn publish_log(&self, log: &LogLine) -> Result<(), AppError> {
        let subject = format!("build.logs.{}", log.deployment_id);
        publish(&self.client, &subject, log).await
    }

    pub async fn get_log_sender(&self, deployment_id: Uuid) -> broadcast::Sender<LogLine> {
        let mut broadcasts = self.log_broadcasts.lock().await;
        broadcasts
            .entry(deployment_id)
            .or_insert_with(|| broadcast::channel::<LogLine>(1024).0)
            .clone()
    }
}

async fn ensure_stream(
    context: &async_nats::jetstream::Context,
    name: &str,
    subjects: Vec<&str>,
) -> Result<Stream, AppError> {
    match context.get_stream(name).await {
        Ok(s) => Ok(s),
        Err(_) => context
            .create_stream(async_nats::jetstream::stream::Config {
                name: name.to_string(),
                subjects: subjects.into_iter().map(|s| s.to_string()).collect(),
                retention: async_nats::jetstream::stream::RetentionPolicy::Limits,
                max_messages: 50_000,
                max_age: std::time::Duration::from_secs(7 * 24 * 3600),
                ..Default::default()
            })
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("failed to create stream {}: {}", name, e))
            }),
    }
}

async fn publish<T: Serialize>(
    client: &async_nats::Client,
    subject: &str,
    data: &T,
) -> Result<(), AppError> {
    let payload = serde_json::to_vec(data).map_err(|e| {
        AppError::Internal(anyhow::anyhow!("failed to serialize NATS message: {}", e))
    })?;

    client
        .publish(subject.to_string(), payload.into())
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to publish to NATS: {}", e)))?;

    Ok(())
}

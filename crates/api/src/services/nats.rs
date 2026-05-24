use async_nats::jetstream::stream::{Config as StreamConfig, Stream};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    config::AppConfig,
    errors::AppError,
    model::build_job::{BuildJob, BuildResult, LogLine},
    services::nats_tls,
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
        let mut opts = async_nats::ConnectOptions::new();
        if let (Some(user), Some(pass)) = (&config.nats_user, &config.nats_password) {
            opts = opts.user_and_password(user.clone(), pass.clone());
        }
        opts = nats_tls::apply_tls(opts, config.nats_tls_ca.as_deref())
            .map_err(|e| AppError::Internal(e))?;
        let client = opts
            .connect(&config.nats_url)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("failed to connect to NATS: {}", e)))?;

        let context = async_nats::jetstream::new(client.clone());

        ensure_stream(&context, "build_jobs", vec!["build.jobs.>", "build.jobs"]).await?;
        ensure_stream(&context, "build_jobs_dlq", vec!["dlq.build.jobs.>"]).await?;
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

    pub async fn subscribe_results(
        &self,
    ) -> Result<impl futures::Stream<Item = crate::model::build_job::BuildResult>, AppError> {
        use futures::StreamExt;

        let stream = self
            .context
            .get_stream("build_results")
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("get build_results stream: {}", e)))?;

        let consumer = stream
            .create_consumer(result_consumer_config())
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("create result consumer: {}", e)))?;

        let messages = consumer
            .messages()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("open result stream: {}", e)))?;

        let out = async_stream::stream! {
            tokio::pin!(messages);
            while let Some(msg) = messages.next().await {
                if let Ok(msg) = msg {
                    if let Ok(result) =
                        serde_json::from_slice::<crate::model::build_job::BuildResult>(&msg.payload)
                    {
                        msg.ack().await.ok();
                        yield result;
                    }
                }
            }
        };

        Ok(out)
    }

    pub async fn get_log_sender(&self, deployment_id: Uuid) -> broadcast::Sender<LogLine> {
        let mut broadcasts = self.log_broadcasts.lock().await;
        broadcasts
            .entry(deployment_id)
            .or_insert_with(|| broadcast::channel::<LogLine>(1024).0)
            .clone()
    }

    /// Drop the sender for a deployment so all SSE subscribers get RecvError::Closed,
    /// which signals the build is finished.
    pub async fn close_log_sender(&self, deployment_id: Uuid) {
        let mut broadcasts = self.log_broadcasts.lock().await;
        broadcasts.remove(&deployment_id);
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
            .create_stream(stream_config(name, subjects))
            .await
            .map_err(|e| {
                AppError::Internal(anyhow::anyhow!("failed to create stream {}: {}", name, e))
            }),
    }
}

fn stream_config(name: &str, subjects: Vec<&str>) -> StreamConfig {
    StreamConfig {
        name: name.to_string(),
        subjects: subjects.into_iter().map(|s| s.to_string()).collect(),
        retention: async_nats::jetstream::stream::RetentionPolicy::Limits,
        max_messages: 50_000,
        max_age: std::time::Duration::from_secs(7 * 24 * 3600),
        ..Default::default()
    }
}

fn result_consumer_config() -> async_nats::jetstream::consumer::pull::Config {
    async_nats::jetstream::consumer::pull::Config {
        durable_name: Some("api-result-processor".to_string()),
        filter_subject: "build.results.>".to_string(),
        deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All,
        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
        max_deliver: 5,
        backoff: vec![
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(30),
            std::time::Duration::from_secs(120),
        ],
        ..Default::default()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlq_stream_captures_failed_build_jobs() {
        let config = stream_config("build_jobs_dlq", vec!["dlq.build.jobs.>"]);

        assert_eq!(config.name, "build_jobs_dlq");
        assert_eq!(config.subjects, vec!["dlq.build.jobs.>".to_string()]);
    }

    #[test]
    fn result_consumer_uses_backoff_for_retries() {
        let config = result_consumer_config();

        assert_eq!(config.max_deliver, 5);
        assert_eq!(
            config.backoff,
            vec![
                std::time::Duration::from_secs(5),
                std::time::Duration::from_secs(30),
                std::time::Duration::from_secs(120),
            ]
        );
    }
}

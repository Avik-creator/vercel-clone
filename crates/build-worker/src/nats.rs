use async_nats::jetstream::stream::Config as StreamConfig;
use futures::StreamExt;

use crate::models::{BuildJob, BuildResult, LogLine};

#[derive(Clone)]
pub struct WorkerNats {
    client: async_nats::Client,
}

impl WorkerNats {
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        let client = async_nats::connect(url)
            .await
            .map_err(|e| anyhow::anyhow!("failed to connect to NATS: {}", e))?;

        let context = async_nats::jetstream::new(client.clone());

        ensure_stream(&context, "build_jobs", vec!["build.jobs.>", "build.jobs"]).await?;
        ensure_stream(&context, "build_jobs_dlq", vec!["build.jobs.dlq.>"]).await?;
        ensure_stream(
            &context,
            "build_results",
            vec!["build.results.>", "build.results"],
        )
        .await?;
        ensure_stream(&context, "build_logs", vec!["build.logs.>", "build.logs"]).await?;

        Ok(Self { client })
    }

    pub async fn subscribe_jobs(&self) -> anyhow::Result<impl futures::Stream<Item = BuildJob>> {
        let context = async_nats::jetstream::new(self.client.clone());

        let stream = context.get_stream("build_jobs").await?;

        let consumer = stream
            .create_consumer(async_nats::jetstream::consumer::pull::Config {
                durable_name: Some("build-worker".to_string()),
                filter_subject: "build.jobs.>".to_string(),
                deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All,
                ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                max_deliver: 3,
                backoff: vec![
                    std::time::Duration::from_secs(5),
                    std::time::Duration::from_secs(30),
                    std::time::Duration::from_secs(120),
                ],
                ..Default::default()
            })
            .await?;

        let mut messages = consumer.messages().await?;

        let stream = async_stream::stream! {
            while let Some(msg) = messages.next().await {
                if let Ok(msg) = msg {
                    if let Ok(job) = serde_json::from_slice::<BuildJob>(&msg.payload) {
                        msg.ack().await.ok();
                        yield job;
                    }
                }
            }
        };

        Ok(stream)
    }

    pub async fn publish_result(&self, result: &BuildResult) -> anyhow::Result<()> {
        let subject = format!("build.results.{}", result.deployment_id);
        let payload = serde_json::to_vec(result)?;
        self.client.publish(subject, payload.into()).await?;
        Ok(())
    }

    pub async fn publish_log(&self, log: &LogLine) -> anyhow::Result<()> {
        let subject = format!("build.logs.{}", log.deployment_id);
        let payload = serde_json::to_vec(log)?;
        self.client.publish(subject, payload.into()).await?;
        Ok(())
    }
}

async fn ensure_stream(
    context: &async_nats::jetstream::Context,
    name: &str,
    subjects: Vec<&str>,
) -> anyhow::Result<()> {
    if context.get_stream(name).await.is_ok() {
        return Ok(());
    }

    context
        .create_stream(StreamConfig {
            name: name.to_string(),
            subjects: subjects.into_iter().map(|s| s.to_string()).collect(),
            retention: async_nats::jetstream::stream::RetentionPolicy::Limits,
            max_messages: 50_000,
            max_age: std::time::Duration::from_secs(7 * 24 * 3600),
            ..Default::default()
        })
        .await?;

    Ok(())
}

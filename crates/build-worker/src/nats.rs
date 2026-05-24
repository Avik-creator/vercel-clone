use async_nats::jetstream::stream::Config as StreamConfig;
use futures::StreamExt;
use std::path::{Path, PathBuf};

use crate::models::{BuildJob, BuildResult, FailedBuildJob, LogLine};

pub struct ReceivedJob {
    pub job: BuildJob,
    message: async_nats::jetstream::Message,
}

impl ReceivedJob {
    pub async fn ack(self) {
        let _ = self.message.ack().await;
    }

    pub async fn nak(&self) {
        let _ = self.message.ack_with(async_nats::jetstream::AckKind::Nak(None)).await;
    }
}

#[derive(Clone)]
pub struct WorkerNats {
    client: async_nats::Client,
}

impl WorkerNats {
    pub async fn connect(
        url: &str,
        user: Option<&str>,
        password: Option<&str>,
        tls_ca: Option<&str>,
    ) -> anyhow::Result<Self> {
        let mut opts = async_nats::ConnectOptions::new();
        if let (Some(u), Some(p)) = (user, password) {
            opts = opts.user_and_password(u.to_string(), p.to_string());
        }
        opts = apply_tls(opts, tls_ca)?;
        let client = opts
            .connect(url)
            .await
            .map_err(|e| anyhow::anyhow!("failed to connect to NATS: {}", e))?;

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

        Ok(Self { client })
    }

    pub async fn subscribe_jobs(&self) -> anyhow::Result<impl futures::Stream<Item = ReceivedJob>> {
        let context = async_nats::jetstream::new(self.client.clone());

        let stream = context.get_stream("build_jobs").await?;

        let consumer = stream
            .create_consumer(async_nats::jetstream::consumer::pull::Config {
                durable_name: Some("build-worker".to_string()),
                filter_subject: "build.jobs.>".to_string(),
                deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All,
                ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                max_deliver: 3,
                ack_wait: std::time::Duration::from_secs(600),
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
                        yield ReceivedJob { job, message: msg };
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

    pub async fn publish_failed_job(&self, job: &BuildJob, error: &str) -> anyhow::Result<()> {
        let payload = FailedBuildJob {
            job: job.clone(),
            error: error.to_string(),
            failed_at: chrono::Utc::now(),
        };
        let subject = format!("dlq.build.jobs.{}", job.deployment_id);
        self.client
            .publish(subject, serde_json::to_vec(&payload)?.into())
            .await?;
        Ok(())
    }
}

fn apply_tls(opts: async_nats::ConnectOptions, ca_file: Option<&str>) -> anyhow::Result<async_nats::ConnectOptions> {
    let Some(ca_path) = ca_file.filter(|p| !p.is_empty() && Path::new(p).exists()) else {
        return Ok(opts);
    };
    Ok(opts
        .require_tls(true)
        .add_root_certificates(PathBuf::from(ca_path)))
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

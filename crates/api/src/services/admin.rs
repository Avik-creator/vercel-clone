use crate::{
    config::AppConfig,
    errors::AppError,
    model::build_job::BuildJob,
    services::nats::NatsClient,
};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedBuildJob {
    pub job: BuildJob,
    pub error: String,
    pub failed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct FailedJobEntry {
    pub sequence: u64,
    pub subject: String,
    pub job: BuildJob,
    pub error: String,
    pub failed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ReplayResponse {
    pub replayed: bool,
    pub deployment_id: Uuid,
}

pub async fn list_failed_jobs(
    nats: &NatsClient,
    limit: usize,
) -> Result<Vec<FailedJobEntry>, AppError> {
    let stream = nats
        .context
        .get_stream("build_jobs_dlq")
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("get build_jobs_dlq stream: {}", e)))?;

    let consumer = stream
        .get_or_create_consumer(
            "admin-dlq-inspector",
            async_nats::jetstream::consumer::pull::Config {
                durable_name: Some("admin-dlq-inspector".to_string()),
                filter_subject: "dlq.build.jobs.>".to_string(),
                deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All,
                ack_policy: async_nats::jetstream::consumer::AckPolicy::None,
                ..Default::default()
            },
        )
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("create DLQ consumer: {}", e)))?;

    let mut messages = consumer
        .fetch()
        .max_messages(limit)
        .messages()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("fetch DLQ messages: {}", e)))?;

    let mut entries = Vec::new();
    while let Some(msg) = messages.next().await {
        let msg = msg.map_err(|e| AppError::Internal(anyhow::anyhow!("read DLQ message: {}", e)))?;
        if let Some(entry) = parse_dlq_message(&msg) {
            entries.push(entry);
        }
    }

    Ok(entries)
}

pub async fn replay_failed_job(
    nats: &NatsClient,
    sequence: u64,
) -> Result<ReplayResponse, AppError> {
    let stream = nats
        .context
        .get_stream("build_jobs_dlq")
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("get build_jobs_dlq stream: {}", e)))?;

    let consumer = stream
        .get_or_create_consumer(
            "admin-dlq-replay",
            async_nats::jetstream::consumer::pull::Config {
                durable_name: Some("admin-dlq-replay".to_string()),
                filter_subject: "dlq.build.jobs.>".to_string(),
                deliver_policy: async_nats::jetstream::consumer::DeliverPolicy::All,
                ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                ..Default::default()
            },
        )
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("create DLQ replay consumer: {}", e)))?;

    let mut messages = consumer
        .fetch()
        .max_messages(1000)
        .messages()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("fetch DLQ messages: {}", e)))?;

    while let Some(msg) = messages.next().await {
        let msg = msg.map_err(|e| AppError::Internal(anyhow::anyhow!("read DLQ message: {}", e)))?;
        let info = msg
            .info()
            .map_err(|e| AppError::Internal(anyhow::anyhow!("read DLQ message info: {}", e)))?;
        if info.stream_sequence != sequence {
            continue;
        }

        let entry = parse_dlq_message(&msg)
            .ok_or_else(|| AppError::BadRequest("invalid DLQ message payload".into()))?;

        nats.publish_job(&entry.job).await?;
        msg.ack()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("ack DLQ message: {}", e)))?;

        return Ok(ReplayResponse {
            replayed: true,
            deployment_id: entry.job.deployment_id,
        });
    }

    Err(AppError::NotFound(format!(
        "failed job with sequence {sequence} not found"
    )))
}

fn parse_dlq_message(msg: &async_nats::jetstream::Message) -> Option<FailedJobEntry> {
    let failed = serde_json::from_slice::<FailedBuildJob>(&msg.payload).ok()?;
    let info = msg.info().ok()?;
    Some(FailedJobEntry {
        sequence: info.stream_sequence,
        subject: msg.subject.to_string(),
        job: failed.job,
        error: failed.error,
        failed_at: failed.failed_at,
    })
}

pub fn verify_admin_secret(config: &AppConfig, token: &str) -> Result<(), AppError> {
    if token == config.admin_secret() {
        Ok(())
    } else {
        Err(AppError::Unauthorized("invalid admin token".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_build_job_round_trips_json() {
        let job = BuildJob {
            deployment_id: Uuid::nil(),
            project_id: Uuid::nil(),
            git_url: "https://github.com/example/repo".into(),
            commit_sha: "abc123".into(),
            branch: "main".into(),
            build_command: None,
            output_dir: None,
            github_token: None,
            env_vars: Default::default(),
        };
        let payload = FailedBuildJob {
            job: job.clone(),
            error: "build failed".into(),
            failed_at: Utc::now(),
        };
        let encoded = serde_json::to_vec(&payload).unwrap();
        let decoded: FailedBuildJob = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded.job.deployment_id, job.deployment_id);
        assert_eq!(decoded.error, "build failed");
    }
}

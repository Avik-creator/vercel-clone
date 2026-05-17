use aws_sdk_s3::primitives::ByteStream;
use std::path::Path;
use walkdir::WalkDir;

use crate::models::LogLine;
use crate::nats::WorkerNats;

const MAX_ARTIFACT_SIZE_BYTES: u64 = 500 * 1024 * 1024;
const MAX_SINGLE_FILE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Clone)]
pub struct Storage {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl Storage {
    pub async fn new(
        endpoint: &str,
        access_key: &str,
        secret_key: &str,
        bucket: &str,
    ) -> anyhow::Result<Self> {
        let credentials =
            aws_sdk_s3::config::Credentials::new(access_key, secret_key, None, None, "worker");

        let config = aws_sdk_s3::config::Builder::new()
            .endpoint_url(endpoint)
            .credentials_provider(credentials)
            .region(aws_sdk_s3::config::Region::new("us-east-1"))
            .force_path_style(true)
            .behavior_version_latest()
            .build();

        let client = aws_sdk_s3::Client::from_conf(config);

        let exists = client.head_bucket().bucket(bucket).send().await.is_ok();

        if !exists {
            client.create_bucket().bucket(bucket).send().await?;
        }

        Ok(Self {
            client,
            bucket: bucket.to_string(),
        })
    }

    pub async fn upload_dir(
        &self,
        deployment_id: uuid::Uuid,
        dir: &Path,
        nats: &WorkerNats,
    ) -> anyhow::Result<String> {
        self.upload_dir_with_prefix(deployment_id, dir, "", nats)
            .await
    }

    /// Upload all files under `dir` to `{deployment_id}/{sub_prefix}/{relative_path}`.
    /// Returns the deployment root prefix `{deployment_id}/`.
    pub async fn upload_dir_with_prefix(
        &self,
        deployment_id: uuid::Uuid,
        dir: &Path,
        sub_prefix: &str,
        nats: &WorkerNats,
    ) -> anyhow::Result<String> {
        let root = format!("{}/", deployment_id);
        let prefix = if sub_prefix.is_empty() {
            root.clone()
        } else {
            format!("{}{}/", root, sub_prefix.trim_matches('/'))
        };

        let mut total_bytes: u64 = 0;

        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let metadata = tokio::fs::metadata(path).await?;
            let file_size = metadata.len();

            if file_size > MAX_SINGLE_FILE_BYTES {
                anyhow::bail!(
                    "file {} ({}MB) exceeds 50MB single-file limit",
                    path.display(),
                    file_size / 1024 / 1024
                );
            }

            if total_bytes + file_size > MAX_ARTIFACT_SIZE_BYTES {
                anyhow::bail!(
                    "artifact total size ({}MB) exceeds 500MB limit",
                    (total_bytes + file_size) / 1024 / 1024
                );
            }

            let relative = path.strip_prefix(dir)?;
            let key = format!("{}{}", prefix, relative.to_string_lossy());

            let bytes = tokio::fs::read(path).await?;
            let stream = ByteStream::from(bytes);

            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&key)
                .content_type(content_type_for(path))
                .body(stream)
                .send()
                .await?;

            total_bytes += file_size;

            let log = LogLine {
                deployment_id,
                line: format!("uploaded: {}", key),
                timestamp: chrono::Utc::now(),
            };
            let _ = nats.publish_log(&log).await;
        }

        Ok(root)
    }
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js") | Some("mjs") | Some("cjs") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("webp") => "image/webp",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("txt") => "text/plain; charset=utf-8",
        Some("xml") => "application/xml",
        Some("webmanifest") => "application/manifest+json",
        Some("map") => "application/json",
        _ => "application/octet-stream",
    }
}

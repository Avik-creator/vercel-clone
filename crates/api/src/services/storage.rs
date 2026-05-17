use crate::config::AppConfig;

pub fn create_client(config: &AppConfig) -> aws_sdk_s3::Client {
    let credentials = aws_sdk_s3::config::Credentials::new(
        &config.minio_access_key,
        &config.minio_secret_key,
        None,
        None,
        "api",
    );

    let s3_config = aws_sdk_s3::config::Builder::new()
        .endpoint_url(&config.minio_endpoint)
        .credentials_provider(credentials)
        .region(aws_sdk_s3::config::Region::new("us-east-1"))
        .force_path_style(true)
        .behavior_version_latest()
        .build();

    aws_sdk_s3::Client::from_conf(s3_config)
}

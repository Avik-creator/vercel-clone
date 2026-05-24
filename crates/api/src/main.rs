use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    http::{Method, StatusCode},
};
use std::time::Duration;
use tokio::signal;
use tower_http::{
    cors::{Any, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

mod config;
mod db;
mod errors;
mod middleware;
mod model;
mod models;
mod routes;
mod services;

use crate::{
    config::AppConfig,
    db::Database,
    model::build_job::LogLine,
    services::deployment_servers::DeploymentServers,
    services::nats::NatsClient,
};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: Arc<AppConfig>,
    pub nats: NatsClient,
    pub storage: aws_sdk_s3::Client,
    pub deployment_servers: Arc<DeploymentServers>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider().install_default();

    dotenvy::dotenv().ok();

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,control_plane=debug,sqlx=warn"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    let config = Arc::new(AppConfig::load()?);
    tracing::info!(env = %config.env, "starting control-plane");

    let db = Database::connect(&config.database_url).await?;
    db.run_migrations().await?;

    let nats = NatsClient::connect(&config).await?;
    tracing::info!(url = %config.nats_url, "nats connected");

    let storage = {
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
    };
    tracing::info!(endpoint = %config.minio_endpoint, "minio client initialized");

    let state = AppState {
        db,
        config: config.clone(),
        nats,
        storage,
        deployment_servers: Arc::new(DeploymentServers::new(
            PathBuf::from("/tmp/vercel-clone-deployments"),
            config.serve_network.clone(),
            300,
            config.serve_tls,
        )),
    };

    tokio::fs::create_dir_all("/tmp/vercel-clone-deployments").await?;

    let servers_for_cleanup = state.deployment_servers.clone();
    tokio::spawn(supervised("idle-cleanup", move || {
        let servers = servers_for_cleanup.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                servers.cleanup_idle().await;
            }
        }
    }));

    let nats_for_logs = state.nats.clone();
    let db_for_logs = state.db.clone();
    tokio::spawn(supervised("log-subscriber", move || {
        let nats = nats_for_logs.clone();
        let db = db_for_logs.clone();
        async move { subscribe_all_logs(nats, db).await }
    }));

    let nats_for_results = state.nats.clone();
    let db_for_results = state.db.clone();
    let servers_for_results = state.deployment_servers.clone();
    tokio::spawn(supervised("build-result-subscriber", move || {
        let nats = nats_for_results.clone();
        let db = db_for_results.clone();
        let servers = servers_for_results.clone();
        async move { subscribe_build_results(nats, db, servers).await }
    }));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
        ])
        .allow_headers(Any);

    let app = Router::new()
        .merge(routes::router(state.clone()))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(addr = %addr, "listening");

    let shutdown_signal = shutdown_signal();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await?;
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

    tracing::info!("shutdown signal received, draining connections...");
}

async fn supervised<F, Fut>(name: &'static str, mut factory: F)
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    loop {
        let result = factory().await;
        tracing::error!(task = name, ?result, "task exited, restarting in 5s");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn subscribe_all_logs(nats: NatsClient, db: Database) -> anyhow::Result<()> {
    use futures::StreamExt;

    let mut subscriber = nats
        .client
        .subscribe("build.logs.>")
        .await
        .map_err(|e| anyhow::anyhow!("failed to subscribe to build.logs.>: {}", e))?;

    tracing::info!("subscribed to all build logs");

    while let Some(msg) = subscriber.next().await {
        if let Ok(log) = serde_json::from_slice::<LogLine>(&msg.payload) {
            if let Err(e) = persist_log_line(&db, &log).await {
                tracing::error!(deployment_id = %log.deployment_id, error = %e, "failed to persist build log line");
            }
            let sender = nats.get_log_sender(log.deployment_id).await;
            let _ = sender.send(log);
        }
    }

    Ok(())
}

async fn persist_log_line(db: &Database, log: &LogLine) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO build_log_lines (deployment_id, line, timestamp) VALUES ($1, $2, $3)",
    )
    .bind(log.deployment_id)
    .bind(&log.line)
    .bind(log.timestamp)
    .execute(&**db)
    .await?;
    Ok(())
}

async fn subscribe_build_results(
    nats: NatsClient,
    db: Database,
    deployment_servers: Arc<DeploymentServers>,
) -> anyhow::Result<()> {
    use futures::StreamExt;

    let subscriber = nats
        .subscribe_results()
        .await
        .map_err(|e| anyhow::anyhow!("failed to subscribe to build results: {}", e))?;

    tracing::info!("subscribed to build results (JetStream)");

    tokio::pin!(subscriber);
    while let Some(result) = subscriber.next().await {
        tracing::info!(
            deployment_id = %result.deployment_id,
            state = ?result.state,
            "received build result"
        );

        let is_terminal = matches!(
            result.state,
            crate::models::DeploymentState::Ready
                | crate::models::DeploymentState::Error
                | crate::models::DeploymentState::Cancelled
        );

        let now = chrono::Utc::now();
        let db_result = match result.state {
            crate::models::DeploymentState::Ready => {
                sqlx::query(
                    "UPDATE deployments SET state = 'ready', build_finished_at = $1, \
                     artifact_key = COALESCE($2, artifact_key), \
                     image_ref = COALESCE($3, image_ref) \
                     WHERE id = $4 AND state IN ('queued', 'building', 'uploading', 'ready')",
                )
                .bind(now)
                .bind(result.artifact_key.as_deref())
                .bind(result.image_ref.as_deref())
                .bind(result.deployment_id)
                .execute(&*db)
                .await
            }
            crate::models::DeploymentState::Error => {
                sqlx::query(
                    "UPDATE deployments SET state = 'error', build_finished_at = $1 \
                     WHERE id = $2 AND state IN ('queued', 'building', 'uploading', 'error')",
                )
                .bind(now)
                .bind(result.deployment_id)
                .execute(&*db)
                .await
            }
            crate::models::DeploymentState::Cancelled => {
                sqlx::query(
                    "UPDATE deployments SET state = 'cancelled', build_finished_at = $1 \
                     WHERE id = $2 AND state IN ('queued', 'building', 'uploading', 'cancelled')",
                )
                .bind(now)
                .bind(result.deployment_id)
                .execute(&*db)
                .await
            }
            crate::models::DeploymentState::Building => {
                sqlx::query(
                    "UPDATE deployments SET state = 'building', build_started_at = NOW() \
                     WHERE id = $1 AND state IN ('queued', 'building')",
                )
                .bind(result.deployment_id)
                .execute(&*db)
                .await
            }
            crate::models::DeploymentState::Uploading => {
                sqlx::query(
                    "UPDATE deployments SET state = 'uploading' \
                     WHERE id = $1 AND state IN ('queued', 'building', 'uploading')",
                )
                .bind(result.deployment_id)
                .execute(&*db)
                .await
            }
            _ => continue,
        };

        if let Err(e) = db_result {
            tracing::error!(error = %e, "failed to update deployment state");
        }

        if matches!(result.state, crate::models::DeploymentState::Ready) {
            if let Some(image_ref) = result.image_ref.as_deref() {
                if let Some(host) = deployment_host(&db, result.deployment_id).await? {
                    let runtime_env = crate::services::deployments::runtime_env_for_deployment(
                        &db,
                        result.deployment_id,
                    )
                    .await
                    .unwrap_or_default();
                    if let Err(e) = deployment_servers
                        .start_image(
                            result.deployment_id,
                            image_ref,
                            &host,
                            &runtime_env,
                        )
                        .await
                    {
                        tracing::error!(deployment_id = %result.deployment_id, error = %e, "failed to start deployment container");
                    }
                }
            }
        }

        if is_terminal {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

            if let Err(e) = sqlx::query(
                "UPDATE deployments SET build_log = (
                    SELECT string_agg(line, E'\\n' ORDER BY id)
                    FROM build_log_lines
                    WHERE deployment_id = $1
                ) WHERE id = $1",
            )
            .bind(result.deployment_id)
            .execute(&*db)
            .await
            {
                tracing::error!(error = %e, "failed to persist build log");
            }

            nats.close_log_sender(result.deployment_id).await;
        }
    }

    Ok(())
}

async fn deployment_host(db: &Database, deployment_id: uuid::Uuid) -> anyhow::Result<Option<String>> {
    let host = sqlx::query_scalar::<_, Option<String>>("SELECT url FROM deployments WHERE id = $1")
        .bind(deployment_id)
        .fetch_one(&**db)
        .await?;
    Ok(host)
}

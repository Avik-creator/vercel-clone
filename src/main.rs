use std::sync::Arc;

use axum::{
    Router,
    http::{Method, StatusCode},
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
    request_id::{MakeRequestUuid, SetRequestIdLayer, PropagateRequestIdLayer},
    timeout::TimeoutLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use std::time::Duration;

mod config;
mod db;
mod errors;
mod middleware;
mod model;
mod models;
mod routes;
mod services;

use crate::{config::AppConfig, db::Database, model::build_job::LogLine, services::nats::NatsClient};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: Arc<AppConfig>,
    pub nats: NatsClient,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let state = AppState { db, config: config.clone(), nats };

    let nats_for_logs = state.nats.clone();
    tokio::spawn(async move {
        if let Err(e) = subscribe_all_logs(nats_for_logs).await {
            tracing::error!(error = %e, "log subscriber failed");
        }
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
        .allow_headers(Any);

    let app = Router::new()
        .merge(routes::router(state.clone()))
        .layer(TimeoutLayer::with_status_code(StatusCode::GATEWAY_TIMEOUT, Duration::from_secs(30)))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid));

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(addr = %addr, "listening");

    axum::serve(listener, app).await?;
    Ok(())
}

async fn subscribe_all_logs(nats: NatsClient) -> anyhow::Result<()> {
    use futures::StreamExt;

    let mut subscriber = nats
        .client
        .subscribe("build.logs.>")
        .await
        .map_err(|e| anyhow::anyhow!("failed to subscribe to build.logs.>: {}", e))?;

    tracing::info!("subscribed to all build logs");

    while let Some(msg) = subscriber.next().await {
        if let Ok(log) = serde_json::from_slice::<LogLine>(&msg.payload) {
            let sender = nats.get_log_sender(log.deployment_id);
            let _ = sender.send(log);
        }
    }

    Ok(())
}

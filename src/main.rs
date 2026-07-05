mod api;
mod core;
mod telemetry;

use anyhow::Context;
use axum::Router;
use clap::Parser;
use core::{config::AppConfig, state::AppState};
use std::{net::SocketAddr, path::PathBuf, sync::{atomic::AtomicU64, Arc}};
use telemetry::store::TelemetryStore;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Parser)]
#[command(name = "lite-api-server")]
#[command(about = "Lightweight Rust LLM API gateway")]
struct Args {
    #[arg(long, default_value = "config.example.yaml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let config = AppConfig::load(&args.config)
        .with_context(|| format!("failed to load config: {}", args.config.display()))?;

    let bind: SocketAddr = config
        .server
        .bind
        .parse()
        .with_context(|| format!("invalid bind address: {}", config.server.bind))?;

    let telemetry = TelemetryStore::connect(&config.telemetry.sqlite_path).await?;
    telemetry.migrate().await?;

    let state = AppState {
        config_path: args.config,
        config: Arc::new(RwLock::new(config)),
        telemetry,
        http: reqwest::Client::new(),
        routing_cursor: Arc::new(AtomicU64::new(0)),
    };

    let app: Router = api::router()
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    tracing::info!("lite-api-server listening on http://{}", bind);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

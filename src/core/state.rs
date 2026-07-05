use crate::{core::config::AppConfig, telemetry::store::TelemetryStore};
use std::{path::PathBuf, sync::{atomic::{AtomicU64, Ordering}, Arc}};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config_path: PathBuf,
    pub config: Arc<RwLock<AppConfig>>,
    pub telemetry: TelemetryStore,
    pub http: reqwest::Client,
    pub routing_cursor: Arc<AtomicU64>,
}

impl AppState {
    pub fn next_routing_cursor(&self) -> u64 {
        self.routing_cursor.fetch_add(1, Ordering::Relaxed)
    }
}

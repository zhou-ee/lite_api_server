use crate::{core::config::AppConfig, telemetry::store::TelemetryStore};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config_path: PathBuf,
    pub config: Arc<RwLock<AppConfig>>,
    pub telemetry: TelemetryStore,
    pub http: reqwest::Client,
}

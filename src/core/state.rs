use crate::{core::config::AppConfig, telemetry::store::TelemetryStore};
use std::{collections::HashMap, path::PathBuf, sync::{Arc, Mutex}};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config_path: PathBuf,
    pub config: Arc<RwLock<AppConfig>>,
    pub telemetry: TelemetryStore,
    pub http: reqwest::Client,
    pub route_cursors: Arc<Mutex<HashMap<String, u64>>>,
}

impl AppState {
    pub fn next_routing_cursor(&self, route_key: &str) -> u64 {
        let mut cursors = self.route_cursors.lock().unwrap();
        let entry = cursors.entry(route_key.to_string()).or_insert(0);
        let val = *entry;
        *entry = entry.wrapping_add(1);
        val
    }
}

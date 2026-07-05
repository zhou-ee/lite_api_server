use super::provider::ProviderConfig;
use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub telemetry: TelemetryConfig,
    pub providers: Vec<ProviderConfig>,
    pub aliases: HashMap<String, String>,
    pub routes: HashMap<String, RouteConfig>,
    pub clients: HashMap<String, ClientConfig>,
    #[serde(default)]
    pub pricing: HashMap<String, PricingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
    pub admin_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub sqlite_path: String,
    pub save_bodies: bool,
    pub retention_days: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    pub strategy: String,
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub api_key: String,
    pub allowed_models: Vec<String>,
    #[serde(default)]
    pub max_daily_requests: Option<i64>,
    #[serde(default)]
    pub max_daily_tokens: Option<i64>,
    #[serde(default)]
    pub max_daily_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
}

impl AppConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Ok(serde_yaml::from_str(&text)?)
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let text = serde_yaml::to_string(self)?;
        std::fs::write(path, text)
            .with_context(|| format!("failed to write {}", path.display()))?;
        Ok(())
    }

    pub fn provider(&self, id: &str) -> anyhow::Result<&ProviderConfig> {
        self.providers
            .iter()
            .find(|p| p.id == id && p.enabled)
            .ok_or_else(|| anyhow!("provider not found or disabled: {id}"))
    }

    pub fn resolve_alias<'a>(&'a self, model: &'a str) -> &'a str {
        self.aliases.get(model).map(|s| s.as_str()).unwrap_or(model)
    }

    pub fn validate_client_key(&self, token: &str, requested_model: &str) -> Option<String> {
        self.clients.iter().find_map(|(name, client)| {
            if client.api_key != token {
                return None;
            }

            if client.allowed_models.iter().any(|m| m == "*" || m == requested_model) {
                Some(name.clone())
            } else {
                None
            }
        })
    }

    pub fn client(&self, name: &str) -> Option<&ClientConfig> {
        self.clients.get(name)
    }

    pub fn estimate_price(
        &self,
        upstream_model: &str,
        input_tokens: Option<i64>,
        output_tokens: Option<i64>,
    ) -> Option<f64> {
        let pricing = self.pricing.get(upstream_model)?;
        let input = input_tokens.unwrap_or_default().max(0) as f64;
        let output = output_tokens.unwrap_or_default().max(0) as f64;
        Some((input / 1_000_000.0) * pricing.input_per_1m + (output / 1_000_000.0) * pricing.output_per_1m)
    }

    pub fn upsert_provider(&mut self, provider: ProviderConfig) {
        if let Some(existing) = self.providers.iter_mut().find(|p| p.id == provider.id) {
            *existing = provider;
        } else {
            self.providers.push(provider);
        }
    }

    pub fn delete_provider(&mut self, id: &str) -> bool {
        let before = self.providers.len();
        self.providers.retain(|p| p.id != id);
        before != self.providers.len()
    }
}

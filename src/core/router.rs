use super::config::AppConfig;
use anyhow::{anyhow, bail};

#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub requested_model: String,
    pub upstream_model: String,
    pub provider_ids: Vec<String>,
}

impl RoutePlan {
    pub fn select(config: &AppConfig, requested_model: &str) -> anyhow::Result<Self> {
        let upstream_model = config.resolve_alias(requested_model).to_string();
        let route = config
            .routes
            .get(&upstream_model)
            .ok_or_else(|| anyhow!("no route configured for model: {requested_model} -> {upstream_model}"))?;

        if route.providers.is_empty() {
            bail!("route has no providers for model: {upstream_model}");
        }

        let mut provider_ids = route
            .providers
            .iter()
            .filter(|id| config.provider(id).is_ok())
            .cloned()
            .collect::<Vec<_>>();

        if route.strategy == "priority_fallback" {
            provider_ids.sort_by_key(|id| config.provider(id).map(|p| p.priority).unwrap_or(i32::MAX));
        }

        if provider_ids.is_empty() {
            bail!("no enabled provider available for model: {upstream_model}");
        }

        Ok(Self {
            requested_model: requested_model.to_string(),
            upstream_model,
            provider_ids,
        })
    }
}

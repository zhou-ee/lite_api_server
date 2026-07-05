use super::config::AppConfig;
use anyhow::{anyhow, bail};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub requested_model: String,
    pub upstream_model: String,
    pub provider_ids: Vec<String>,
}

impl RoutePlan {
    pub fn select(
        config: &AppConfig,
        requested_model: &str,
        provider_latency_ms: &HashMap<String, f64>,
    ) -> anyhow::Result<Self> {
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

        match route.strategy.as_str() {
            "priority_fallback" => {
                provider_ids.sort_by_key(|id| config.provider(id).map(|p| p.priority).unwrap_or(i32::MAX));
            }
            "weighted" => {
                provider_ids.sort_by_key(|id| -config.provider(id).map(|p| p.weight).unwrap_or_default());
            }
            "cheapest" => {
                provider_ids.sort_by(|a, b| {
                    let a_score = model_price_score(config, &upstream_model).unwrap_or(f64::MAX)
                        + config.provider(a).map(|p| p.priority as f64 / 1000.0).unwrap_or(0.0);
                    let b_score = model_price_score(config, &upstream_model).unwrap_or(f64::MAX)
                        + config.provider(b).map(|p| p.priority as f64 / 1000.0).unwrap_or(0.0);
                    a_score.partial_cmp(&b_score).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            "lowest_latency" => {
                provider_ids.sort_by(|a, b| {
                    let a_latency = provider_latency_ms.get(a).copied().unwrap_or(f64::MAX);
                    let b_latency = provider_latency_ms.get(b).copied().unwrap_or(f64::MAX);
                    a_latency
                        .partial_cmp(&b_latency)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| {
                            let a_priority = config.provider(a).map(|p| p.priority).unwrap_or(i32::MAX);
                            let b_priority = config.provider(b).map(|p| p.priority).unwrap_or(i32::MAX);
                            a_priority.cmp(&b_priority)
                        })
                });
            }
            _ => {
                provider_ids.sort_by_key(|id| config.provider(id).map(|p| p.priority).unwrap_or(i32::MAX));
            }
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

fn model_price_score(config: &AppConfig, upstream_model: &str) -> Option<f64> {
    let pricing = config.pricing.get(upstream_model)?;
    Some(pricing.input_per_1m + pricing.output_per_1m)
}

use super::config::AppConfig;
use anyhow::{anyhow, bail};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RoutePlan {
    pub requested_model: String,
    pub upstream_model: String,
    pub provider_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RouteRuntimeHint {
    pub latency_ms: HashMap<String, f64>,
    pub cursor: u64,
    pub seed: u64,
}

impl RoutePlan {
    pub fn select(
        config: &AppConfig,
        requested_model: &str,
        hint: &RouteRuntimeHint,
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
            "round_robin" => {
                provider_ids.sort_by_key(|id| config.provider(id).map(|p| p.priority).unwrap_or(i32::MAX));
                rotate_left_by_cursor(&mut provider_ids, hint.cursor);
            }
            "weighted_random" => {
                provider_ids = weighted_random_order(config, &provider_ids, hint.seed);
            }
            "cheapest" => {
                provider_ids.sort_by(|a, b| {
                    let a_score = config.model_price_score(a, &upstream_model).unwrap_or(f64::MAX)
                        + config.provider(a).map(|p| p.priority as f64 / 1000.0).unwrap_or(0.0);
                    let b_score = config.model_price_score(b, &upstream_model).unwrap_or(f64::MAX)
                        + config.provider(b).map(|p| p.priority as f64 / 1000.0).unwrap_or(0.0);
                    a_score.partial_cmp(&b_score).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            "lowest_latency" => {
                provider_ids.sort_by(|a, b| {
                    let a_latency = hint.latency_ms.get(a).copied().unwrap_or(f64::MAX);
                    let b_latency = hint.latency_ms.get(b).copied().unwrap_or(f64::MAX);
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

fn rotate_left_by_cursor<T>(values: &mut [T], cursor: u64) {
    if values.len() > 1 {
        values.rotate_left((cursor as usize) % values.len());
    }
}

fn weighted_random_order(config: &AppConfig, provider_ids: &[String], seed: u64) -> Vec<String> {
    let mut remaining = provider_ids.to_vec();
    let mut ordered = Vec::with_capacity(remaining.len());
    let mut state = seed.max(1);

    while !remaining.is_empty() {
        let total_weight = remaining
            .iter()
            .map(|id| config.provider(id).map(|p| p.weight.max(1) as u64).unwrap_or(1))
            .sum::<u64>()
            .max(1);

        state = splitmix64(state);
        let mut pick = state % total_weight;
        let mut selected = 0usize;

        for (index, id) in remaining.iter().enumerate() {
            let weight = config.provider(id).map(|p| p.weight.max(1) as u64).unwrap_or(1);
            if pick < weight {
                selected = index;
                break;
            }
            pick -= weight;
        }

        ordered.push(remaining.remove(selected));
    }

    ordered
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

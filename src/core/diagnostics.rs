use crate::core::{config::AppConfig, provider::ProviderKind};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticItem {
    pub level: DiagnosticLevel,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticReport {
    pub ok: bool,
    pub errors: usize,
    pub warnings: usize,
    pub items: Vec<DiagnosticItem>,
}

pub fn analyze_config(config: &AppConfig) -> DiagnosticReport {
    let mut items = Vec::new();

    check_providers(config, &mut items);
    check_routes(config, &mut items);
    check_aliases(config, &mut items);
    check_clients(config, &mut items);
    check_pricing(config, &mut items);

    let errors = items.iter().filter(|item| matches!(item.level, DiagnosticLevel::Error)).count();
    let warnings = items.iter().filter(|item| matches!(item.level, DiagnosticLevel::Warning)).count();

    DiagnosticReport {
        ok: errors == 0,
        errors,
        warnings,
        items,
    }
}

fn push(items: &mut Vec<DiagnosticItem>, level: DiagnosticLevel, code: &str, message: impl Into<String>) {
    items.push(DiagnosticItem {
        level,
        code: code.to_string(),
        message: message.into(),
    });
}

fn check_providers(config: &AppConfig, items: &mut Vec<DiagnosticItem>) {
    if config.providers.is_empty() {
        push(items, DiagnosticLevel::Error, "no_providers", "No providers configured.");
        return;
    }

    let mut seen = HashSet::new();
    for provider in &config.providers {
        if provider.id.trim().is_empty() {
            push(items, DiagnosticLevel::Error, "provider_empty_id", "A provider has an empty id.");
        }
        if !seen.insert(provider.id.clone()) {
            push(items, DiagnosticLevel::Error, "provider_duplicate_id", format!("Duplicate provider id: {}", provider.id));
        }
        if provider.base_url.trim().is_empty() {
            push(items, DiagnosticLevel::Error, "provider_empty_base_url", format!("Provider {} has an empty base_url.", provider.id));
        }
        if provider.models.is_empty() {
            push(items, DiagnosticLevel::Warning, "provider_no_models", format!("Provider {} has no declared models.", provider.id));
        }
        if provider.kind != ProviderKind::OpenaiCompatible {
            push(items, DiagnosticLevel::Warning, "provider_kind_not_supported_yet", format!("Provider {} uses a kind that is not fully implemented yet.", provider.id));
        }
    }
}

fn check_routes(config: &AppConfig, items: &mut Vec<DiagnosticItem>) {
    if config.routes.is_empty() {
        push(items, DiagnosticLevel::Error, "no_routes", "No model routes configured.");
        return;
    }

    let provider_ids = config.providers.iter().map(|p| p.id.as_str()).collect::<HashSet<_>>();
    let supported_strategies = ["priority_fallback", "weighted", "cheapest", "lowest_latency"];

    for (model, route) in &config.routes {
        if route.providers.is_empty() {
            push(items, DiagnosticLevel::Error, "route_no_providers", format!("Route {} has no providers.", model));
        }
        if !supported_strategies.contains(&route.strategy.as_str()) {
            push(items, DiagnosticLevel::Warning, "route_unknown_strategy", format!("Route {} uses unknown strategy {}.", model, route.strategy));
        }
        for provider_id in &route.providers {
            if !provider_ids.contains(provider_id.as_str()) {
                push(items, DiagnosticLevel::Error, "route_unknown_provider", format!("Route {} references unknown provider {}.", model, provider_id));
            }
        }
    }
}

fn check_aliases(config: &AppConfig, items: &mut Vec<DiagnosticItem>) {
    for (alias, target) in &config.aliases {
        if alias == target {
            push(items, DiagnosticLevel::Warning, "alias_self_reference", format!("Alias {} points to itself.", alias));
        }
        if !config.routes.contains_key(target) {
            push(items, DiagnosticLevel::Warning, "alias_target_missing_route", format!("Alias {} targets {}, but no route exists for that target.", alias, target));
        }
    }
}

fn check_clients(config: &AppConfig, items: &mut Vec<DiagnosticItem>) {
    if config.clients.is_empty() {
        push(items, DiagnosticLevel::Warning, "no_clients", "No client entries configured.");
    }

    let known_models = config.routes.keys().cloned().chain(config.aliases.keys().cloned()).collect::<HashSet<_>>();
    for (name, client) in &config.clients {
        if client.allowed_models.is_empty() {
            push(items, DiagnosticLevel::Warning, "client_no_models", format!("Client {} has no allowed models.", name));
        }
        for model in &client.allowed_models {
            if model != "*" && !known_models.contains(model) {
                push(items, DiagnosticLevel::Warning, "client_unknown_model", format!("Client {} allows {}, but it is not a known alias or route.", name, model));
            }
        }
    }
}

fn check_pricing(config: &AppConfig, items: &mut Vec<DiagnosticItem>) {
    for model in config.routes.keys() {
        if !config.pricing.contains_key(model) {
            push(items, DiagnosticLevel::Info, "pricing_missing", format!("No pricing configured for model {}.", model));
        }
    }

    for model in config.pricing.keys() {
        if !config.routes.contains_key(model) {
            push(items, DiagnosticLevel::Info, "pricing_unused", format!("Pricing is configured for {}, but no route currently uses it.", model));
        }
    }
}

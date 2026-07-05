use crate::{
    core::{diagnostics::analyze_config, router::{RoutePlan, RouteRuntimeHint}, state::AppState},
};
use axum::{extract::{Query, State}, http::{HeaderMap, StatusCode}, Json};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct RoutePreviewQuery {
    pub model: String,
}

pub async fn get_diagnostics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<crate::core::diagnostics::DiagnosticReport>, StatusCode> {
    require_management_access(&state, &headers).await?;
    let config = state.config.read().await;
    Ok(Json(analyze_config(&config)))
}

pub async fn preview_route(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RoutePreviewQuery>,
) -> Result<Json<Value>, StatusCode> {
    require_management_access(&state, &headers).await?;
    let latency = state.telemetry.provider_latency_snapshot().await.unwrap_or_default();
    let hint = RouteRuntimeHint {
        latency_ms: latency.clone(),
        cursor: state.next_routing_cursor(),
        seed: 1,
    };
    let config = state.config.read().await;

    match RoutePlan::select(&config, &query.model, &hint) {
        Ok(plan) => Ok(Json(json!({
            "ok": true,
            "requested_model": plan.requested_model,
            "upstream_model": plan.upstream_model,
            "provider_order": plan.provider_ids,
            "latency_snapshot_ms": latency
        }))),
        Err(error) => Ok(Json(json!({
            "ok": false,
            "requested_model": query.model,
            "error": error.to_string()
        }))),
    }
}

async fn require_management_access(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let config = state.config.read().await;
    let Some(value) = headers.get("authorization").and_then(|v| v.to_str().ok()) else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let expected = format!("Bearer {}", config.server.admin_token);
    if value == expected {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

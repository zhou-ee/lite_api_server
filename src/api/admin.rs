use crate::{
    core::{
        config::{AppConfig, RouteConfig},
        provider::{ProviderConfig, ProviderKind},
        state::AppState,
    },
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashMap, time::Instant};

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    pub limit: Option<i64>,
}

pub async fn get_config(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AppConfig>, StatusCode> {
    require_admin(&state, &headers).await?;
    Ok(Json(state.config.read().await.clone()))
}

pub async fn put_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(new_config): Json<AppConfig>,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;

    {
        let mut config = state.config.write().await;
        *config = new_config.clone();
    }

    new_config
        .save(&state.config_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({"ok": true, "message": "config updated"})))
}

pub async fn get_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LogQuery>,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let logs = state
        .telemetry
        .recent_logs(query.limit.unwrap_or(100))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({"data": logs})))
}

pub async fn stats_today(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let stats = state
        .telemetry
        .today_stats()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(stats))
}

pub async fn stats_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let stats = state
        .telemetry
        .provider_stats()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(stats))
}

pub async fn stats_models(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let stats = state
        .telemetry
        .model_stats()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(stats))
}

pub async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let config = state.config.read().await;
    Ok(Json(json!({"data": config.providers.clone()})))
}

pub async fn upsert_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(provider): Json<ProviderConfig>,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    upsert_and_save(&state, provider).await
}

pub async fn upsert_provider_by_id(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut provider): Json<ProviderConfig>,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    provider.id = id;
    upsert_and_save(&state, provider).await
}

pub async fn delete_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;

    let saved_config = {
        let mut config = state.config.write().await;
        let deleted = config.delete_provider(&id);
        if !deleted {
            return Err(StatusCode::NOT_FOUND);
        }
        config.clone()
    };

    saved_config
        .save(&state.config_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({"ok": true, "deleted": id})))
}

pub async fn healthcheck_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;

    let provider = {
        let config = state.config.read().await;
        config.provider(&id).map_err(|_| StatusCode::NOT_FOUND)?.clone()
    };

    let started = Instant::now();
    let result = match provider.kind {
        ProviderKind::OpenaiCompatible => {
            let url = format!("{}/models", provider.base_url.trim_end_matches('/'));
            state
                .http
                .get(url)
                .timeout(std::time::Duration::from_millis(provider.timeout_ms.min(15_000)))
                .bearer_auth(provider.api_key)
                .send()
                .await
        }
        _ => {
            return Ok(Json(json!({
                "ok": false,
                "provider_id": id,
                "error": "healthcheck is not implemented for this provider kind yet"
            })));
        }
    };

    let latency_ms = started.elapsed().as_millis() as i64;
    match result {
        Ok(resp) => Ok(Json(json!({
            "ok": resp.status().is_success(),
            "provider_id": id,
            "status": resp.status().as_u16(),
            "latency_ms": latency_ms
        }))),
        Err(err) => Ok(Json(json!({
            "ok": false,
            "provider_id": id,
            "error": err.to_string(),
            "latency_ms": latency_ms
        }))),
    }
}

pub async fn list_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let config = state.config.read().await;
    Ok(Json(json!({"data": config.routes.clone()})))
}

pub async fn put_routes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(routes): Json<HashMap<String, RouteConfig>>,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;

    let saved_config = {
        let mut config = state.config.write().await;
        config.routes = routes;
        config.clone()
    };

    saved_config
        .save(&state.config_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({"ok": true, "message": "routes updated"})))
}

async fn upsert_and_save(state: &AppState, provider: ProviderConfig) -> Result<Json<Value>, StatusCode> {
    let provider_id = provider.id.clone();
    let saved_config = {
        let mut config = state.config.write().await;
        config.upsert_provider(provider);
        config.clone()
    };

    saved_config
        .save(&state.config_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({"ok": true, "provider_id": provider_id})))
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let config = state.config.read().await;
    let expected = format!("Bearer {}", config.server.admin_token);
    let got = headers.get("authorization").and_then(|v| v.to_str().ok());

    match got {
        Some(v) if v == expected => Ok(()),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

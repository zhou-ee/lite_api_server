use crate::{
    core::{
        config::{AppConfig, RouteConfig},
        provider::ProviderConfig,
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
use std::collections::HashMap;

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

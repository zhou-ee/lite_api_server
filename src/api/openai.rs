use crate::{
    api::oauth::refresh_provider_token_if_needed,
    core::{provider::ProviderKind, router::{RoutePlan, RouteRuntimeHint}, state::AppState},
    telemetry::store::RequestLog,
};
use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bytes::Bytes;
use chrono::Utc;
use reqwest::Response as UpstreamResponse;
use serde_json::{json, Value};
use std::{collections::hash_map::DefaultHasher, hash::{Hash, Hasher}, time::Instant};
use uuid::Uuid;

pub async fn models(State(state): State<AppState>) -> Json<Value> {
    let config = state.config.read().await;
    let aliases = config.aliases.keys().cloned();
    let routes = config.routes.keys().cloned();

    Json(json!({
        "object": "list",
        "data": aliases
            .chain(routes)
            .map(|id| json!({"id": id, "object": "model", "owned_by": "lite-api-server"}))
            .collect::<Vec<_>>()
    }))
}

pub async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut payload): Json<Value>,
) -> Response {
    let started = Instant::now();
    let request_id = Uuid::new_v4().to_string();

    let requested_model = payload
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let wants_stream = payload.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

    let client_name = match authorize_client(&state, &headers, &requested_model).await {
        Ok(name) => name,
        Err(response) => return response,
    };

    if let Err(response) = enforce_client_limits(&state, client_name.as_deref()).await {
        return response;
    }

    let latency_snapshot = state.telemetry.provider_latency_snapshot().await.unwrap_or_default();
    let upstream_model = {
        let config = state.config.read().await;
        config.resolve_alias(&requested_model).to_string()
    };
    let hint = RouteRuntimeHint {
        latency_ms: latency_snapshot,
        cursor: state.next_routing_cursor(&upstream_model),
        seed: request_seed(&request_id, &requested_model),
    };
    let plan = {
        let config = state.config.read().await;
        match RoutePlan::select(&config, &requested_model, &hint) {
            Ok(plan) => plan,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": {"message": e.to_string(), "type": "routing_error"}
                    })),
                )
                    .into_response();
            }
        }
    };

    payload["model"] = Value::String(plan.upstream_model.clone());

    let mut last_error: Option<String> = None;
    let mut last_provider = String::from("none");

    for provider_id in &plan.provider_ids {
        last_provider = provider_id.clone();
        let provider = {
            let config = state.config.read().await;
            match config.provider(provider_id) {
                Ok(provider) => provider.clone(),
                Err(e) => {
                    last_error = Some(e.to_string());
                    continue;
                }
            }
        };
        let provider = refresh_provider_token_if_needed(&state, provider).await;

        if provider.kind != ProviderKind::OpenaiCompatible {
            last_error = Some(format!("provider kind is not supported by /v1/chat/completions yet: {:?}", provider.kind));
            continue;
        }

        match send_openai_compatible(&state, &provider, payload.clone()).await {
            Ok(upstream) => {
                let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
                let retryable = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
                if retryable && provider_id != plan.provider_ids.last().unwrap() {
                    last_error = Some(format!("upstream returned retryable status: {status}"));
                    continue;
                }

                if wants_stream {
                    return stream_response(
                        state.clone(),
                        upstream,
                        request_id,
                        client_name,
                        provider.id,
                        plan.requested_model,
                        plan.upstream_model,
                        plan.strategy,
                        started,
                    );
                }

                match buffered_response(upstream).await {
                    Ok((status, content_type, body)) => {
                        let latency_ms = started.elapsed().as_millis() as i64;
                        let usage = parse_usage(&body);
                        let estimated_price = {
                            let config = state.config.read().await;
                            config.estimate_price(&provider.id, &plan.upstream_model, usage.0, usage.1)
                        };

                        let _ = state
                            .telemetry
                            .record_request(RequestLog {
                                id: request_id.clone(),
                                ts: Utc::now().timestamp(),
                                client_name: client_name.clone(),
                                provider_id: provider.id.clone(),
                                requested_model: plan.requested_model.clone(),
                                upstream_model: plan.upstream_model.clone(),
                                status_code: status.as_u16() as i64,
                                error_type: if status.is_success() { None } else { Some(status.to_string()) },
                                latency_ms,
                                input_tokens: usage.0,
                                output_tokens: usage.1,
                                total_tokens: usage.2,
                                estimated_cost_usd: estimated_price,
                                route_strategy: Some(plan.strategy.clone()),
                            })
                            .await;

                        let mut res = Response::new(Body::from(body));
                        *res.status_mut() = status;
                        if let Some(ct) = content_type {
                            res.headers_mut().insert("content-type", ct);
                        }
                        add_gateway_headers(
                            &mut res,
                            &request_id,
                            &provider.id,
                            &plan.strategy,
                            &plan.upstream_model,
                        );
                        return res;
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                        continue;
                    }
                }
            }
            Err(e) => {
                last_error = Some(e.to_string());
                continue;
            }
        }
    }

    let latency_ms = started.elapsed().as_millis() as i64;
    let error = last_error.unwrap_or_else(|| "all providers failed".to_string());
    let _ = state
        .telemetry
        .record_request(RequestLog {
            id: request_id.clone(),
            ts: Utc::now().timestamp(),
            client_name,
            provider_id: last_provider,
            requested_model: plan.requested_model,
            upstream_model: plan.upstream_model,
            status_code: 502,
            error_type: Some(error.clone()),
            latency_ms,
            input_tokens: None,
            output_tokens: None,
            total_tokens: None,
            estimated_cost_usd: None,
            route_strategy: Some(plan.strategy),
        })
        .await;

    (
        StatusCode::BAD_GATEWAY,
        Json(json!({
            "error": {
                "message": error,
                "type": "upstream_error",
                "request_id": request_id
            }
        })),
    )
        .into_response()
}

async fn authorize_client(
    state: &AppState,
    headers: &HeaderMap,
    requested_model: &str,
) -> Result<Option<String>, Response> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or_default();

    let config = state.config.read().await;
    if let Some(name) = config.validate_client_key(token, requested_model) {
        return Ok(Some(name));
    }

    Err((
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": {
                "message": "invalid client API key or model not allowed",
                "type": "auth_error"
            }
        })),
    )
        .into_response())
}

async fn enforce_client_limits(state: &AppState, client_name: Option<&str>) -> Result<(), Response> {
    let Some(client_name) = client_name else {
        return Ok(());
    };

    let limits = {
        let config = state.config.read().await;
        let Some(client) = config.client(client_name) else {
            return Ok(());
        };
        (
            client.max_daily_requests,
            client.max_daily_tokens,
            client.max_daily_cost_usd,
        )
    };

    if limits.0.is_none() && limits.1.is_none() && limits.2.is_none() {
        return Ok(());
    }

    let usage = match state.telemetry.client_usage_today(client_name).await {
        Ok(usage) => usage,
        Err(error) => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "message": format!("failed to read client usage: {error}"),
                        "type": "usage_check_error"
                    }
                })),
            )
                .into_response());
        }
    };

    let blocked = limits.0.is_some_and(|limit| usage.request_count >= limit)
        || limits.1.is_some_and(|limit| usage.total_tokens >= limit)
        || limits.2.is_some_and(|limit| usage.estimated_cost_usd >= limit);

    if blocked {
        Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "error": {
                    "message": "client daily limit reached",
                    "type": "client_limit_reached"
                }
            })),
        )
            .into_response())
    } else {
        Ok(())
    }
}

async fn send_openai_compatible(
    state: &AppState,
    provider: &crate::core::provider::ProviderConfig,
    payload: Value,
) -> anyhow::Result<UpstreamResponse> {
    let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
    let timeout = std::time::Duration::from_millis(provider.timeout_ms);

    Ok(state
        .http
        .post(url)
        .timeout(timeout)
        .bearer_auth(provider.api_key.clone())
        .json(&payload)
        .send()
        .await?)
}

async fn buffered_response(upstream: UpstreamResponse) -> anyhow::Result<(StatusCode, Option<HeaderValue>, Bytes)> {
    let status = StatusCode::from_u16(upstream.status().as_u16())?;
    let content_type = upstream.headers().get("content-type").cloned();
    let body = upstream.bytes().await?;
    Ok((status, content_type, body))
}

fn stream_response(
    state: AppState,
    upstream: UpstreamResponse,
    request_id: String,
    client_name: Option<String>,
    provider_id: String,
    requested_model: String,
    upstream_model: String,
    strategy: String,
    started: Instant,
) -> Response {
    let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = upstream.headers().get("content-type").cloned();

    let telemetry = state.telemetry.clone();
    let request_id_for_log = request_id.clone();
    let provider_id_for_log = provider_id.clone();
    let strategy_for_log = strategy.clone();
    let upstream_model_for_log = upstream_model.clone();
    tokio::spawn(async move {
        let _ = telemetry
            .record_request(RequestLog {
                id: request_id_for_log,
                ts: Utc::now().timestamp(),
                client_name,
                provider_id: provider_id_for_log,
                requested_model,
                upstream_model: upstream_model_for_log,
                status_code: status.as_u16() as i64,
                error_type: if status.is_success() { None } else { Some(status.to_string()) },
                latency_ms: started.elapsed().as_millis() as i64,
                input_tokens: None,
                output_tokens: None,
                total_tokens: None,
                estimated_cost_usd: None,
                route_strategy: Some(strategy_for_log),
            })
            .await;
    });

    let mut res = Response::new(Body::from_stream(upstream.bytes_stream()));
    *res.status_mut() = status;
    if let Some(ct) = content_type {
        res.headers_mut().insert("content-type", ct);
    }
    add_gateway_headers(
        &mut res,
        &request_id,
        &provider_id,
        &strategy,
        &upstream_model,
    );
    res
}

fn add_gateway_headers(
    res: &mut Response,
    request_id: &str,
    provider_id: &str,
    route_strategy: &str,
    upstream_model: &str,
) {
    if let Ok(value) = HeaderValue::from_str(request_id) {
        res.headers_mut().insert("x-lite-api-request-id", value);
    }
    if let Ok(value) = HeaderValue::from_str(provider_id) {
        res.headers_mut().insert("x-lite-api-provider", value);
    }
    if let Ok(value) = HeaderValue::from_str(route_strategy) {
        res.headers_mut().insert("x-lite-api-route-strategy", value);
    }
    if let Ok(value) = HeaderValue::from_str(upstream_model) {
        res.headers_mut().insert("x-lite-api-upstream-model", value);
    }
}

fn request_seed(request_id: &str, model: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    request_id.hash(&mut hasher);
    model.hash(&mut hasher);
    hasher.finish()
}

fn parse_usage(body: &Bytes) -> (Option<i64>, Option<i64>, Option<i64>) {
    let Ok(v) = serde_json::from_slice::<Value>(body) else {
        return (None, None, None);
    };
    let usage = &v["usage"];
    (
        usage.get("prompt_tokens").and_then(|x| x.as_i64()),
        usage.get("completion_tokens").and_then(|x| x.as_i64()),
        usage.get("total_tokens").and_then(|x| x.as_i64()),
    )
}

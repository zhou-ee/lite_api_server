use crate::{
    core::{provider::ProviderKind, router::RoutePlan, state::AppState},
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
use serde_json::{json, Value};
use std::time::Instant;
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

    let client_name = match authorize_client(&state, &headers, &requested_model).await {
        Ok(name) => name,
        Err(response) => return response,
    };

    let plan = {
        let config = state.config.read().await;
        match RoutePlan::select(&config, &requested_model) {
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

        if provider.kind != ProviderKind::OpenaiCompatible {
            last_error = Some(format!("provider kind is not supported by /v1/chat/completions yet: {:?}", provider.kind));
            continue;
        }

        match proxy_openai_compatible(&state, &provider, payload.clone()).await {
            Ok((status, content_type, body)) => {
                let latency_ms = started.elapsed().as_millis() as i64;
                let usage = parse_usage(&body);

                let retryable = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
                if retryable && provider_id != plan.provider_ids.last().unwrap() {
                    last_error = Some(format!("upstream returned retryable status: {status}"));
                    continue;
                }

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
                        estimated_cost_usd: None,
                    })
                    .await;

                let mut res = Response::new(Body::from(body));
                *res.status_mut() = status;
                if let Some(ct) = content_type {
                    res.headers_mut().insert("content-type", ct);
                }
                res.headers_mut().insert(
                    "x-lite-api-request-id",
                    HeaderValue::from_str(&request_id).unwrap(),
                );
                res.headers_mut().insert(
                    "x-lite-api-provider",
                    HeaderValue::from_str(&provider.id).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
                );
                return res;
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

async fn proxy_openai_compatible(
    state: &AppState,
    provider: &crate::core::provider::ProviderConfig,
    payload: Value,
) -> anyhow::Result<(StatusCode, Option<HeaderValue>, Bytes)> {
    let url = format!("{}/chat/completions", provider.base_url.trim_end_matches('/'));
    let timeout = std::time::Duration::from_millis(provider.timeout_ms);

    let resp = state
        .http
        .post(url)
        .timeout(timeout)
        .bearer_auth(provider.api_key.clone())
        .json(&payload)
        .send()
        .await?;

    let status = StatusCode::from_u16(resp.status().as_u16())?;
    let content_type = resp.headers().get("content-type").cloned();
    let body = resp.bytes().await?;
    Ok((status, content_type, body))
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

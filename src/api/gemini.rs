use crate::core::{provider::ProviderConfig, state::AppState};
use axum::http::{HeaderValue, StatusCode};
use bytes::Bytes;
use chrono::Utc;
use serde_json::{json, Value};

pub async fn send_gemini_as_openai(
    state: &AppState,
    provider: &ProviderConfig,
    upstream_model: &str,
    openai_payload: Value,
) -> anyhow::Result<(StatusCode, Option<HeaderValue>, Bytes)> {
    let url = gemini_generate_url(provider, upstream_model);
    let body = openai_to_gemini_payload(&openai_payload);
    let timeout = std::time::Duration::from_millis(provider.timeout_ms);

    let upstream = state
        .http
        .post(url)
        .timeout(timeout)
        .bearer_auth(provider.api_key.clone())
        .json(&body)
        .send()
        .await?;

    let status = StatusCode::from_u16(upstream.status().as_u16())?;
    let upstream_body = upstream.bytes().await?;
    if !status.is_success() {
        return Ok((status, json_content_type(), upstream_body));
    }

    let gemini_value = serde_json::from_slice::<Value>(&upstream_body)?;
    let normalized = gemini_to_openai_response(upstream_model, &gemini_value);
    Ok((status, json_content_type(), Bytes::from(serde_json::to_vec(&normalized)?)))
}

fn gemini_generate_url(provider: &ProviderConfig, upstream_model: &str) -> String {
    let base = provider.base_url.trim_end_matches('/');
    let model = upstream_model.trim_start_matches("models/");
    if base.ends_with("/v1beta") || base.ends_with("/v1") {
        format!("{base}/models/{model}:generateContent")
    } else {
        format!("{base}/v1beta/models/{model}:generateContent")
    }
}

fn openai_to_gemini_payload(payload: &Value) -> Value {
    let contents = payload
        .get("messages")
        .and_then(|value| value.as_array())
        .map(|messages| messages.iter().filter_map(message_to_content).collect::<Vec<_>>())
        .unwrap_or_default();
    json!({ "contents": contents })
}

fn message_to_content(message: &Value) -> Option<Value> {
    let role = match message.get("role").and_then(|value| value.as_str()).unwrap_or("user") {
        "assistant" => "model",
        _ => "user",
    };
    let text = extract_text(message.get("content")?)?;
    Some(json!({"role": role, "parts": [{"text": text}]}))
}

fn extract_text(value: &Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    value.as_array().map(|items| {
        items
            .iter()
            .filter_map(|item| item.get("text").and_then(|text| text.as_str()).map(str::to_string))
            .collect::<Vec<_>>()
            .join("\n")
    }).filter(|text| !text.is_empty())
}

fn gemini_to_openai_response(upstream_model: &str, value: &Value) -> Value {
    let content = value
        .get("candidates")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|candidate| candidate.get("content"))
        .and_then(|content| content.get("parts"))
        .and_then(|parts| parts.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(|text| text.as_str()))
                .collect::<Vec<_>>()
                .join("")
        })
        .unwrap_or_default();

    let usage = &value["usageMetadata"];
    json!({
        "id": format!("chatcmpl-gemini-{}", Utc::now().timestamp_millis()),
        "object": "chat.completion",
        "created": Utc::now().timestamp(),
        "model": upstream_model,
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": usage.get("promptTokenCount").and_then(|value| value.as_i64()),
            "completion_tokens": usage.get("candidatesTokenCount").and_then(|value| value.as_i64()),
            "total_tokens": usage.get("totalTokenCount").and_then(|value| value.as_i64())
        }
    })
}

fn json_content_type() -> Option<HeaderValue> {
    Some(HeaderValue::from_static("application/json"))
}

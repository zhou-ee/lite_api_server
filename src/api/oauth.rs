use crate::{
    core::{provider::{ProviderConfig, ProviderKind}, state::AppState},
};
use axum::{extract::{Query, State}, http::{HeaderMap, StatusCode}, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v2/userinfo";
const GOOGLE_SCOPES: &str = "openid https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/userinfo.profile";

#[derive(Debug, Deserialize)]
pub struct OAuthStartQuery {
    pub provider_id: Option<String>,
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OAuthExchangeBody {
    pub code: String,
    pub provider_id: Option<String>,
    pub redirect_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OAuthStartResponse {
    pub url: String,
}

pub async fn start_oauth(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<OAuthStartQuery>,
) -> Result<Json<OAuthStartResponse>, StatusCode> {
    require_admin(&state, &headers).await?;
    let client_id = google_client_id().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let redirect_uri = query.redirect_uri.unwrap_or_else(default_redirect_uri);
    let provider_id = query.provider_id.unwrap_or_else(|| "google-oauth".to_string());

    let url = format!(
        "{GOOGLE_AUTH_URL}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}",
        enc(&client_id),
        enc(&redirect_uri),
        enc(GOOGLE_SCOPES),
        enc(&provider_id),
    );

    Ok(Json(OAuthStartResponse { url }))
}

pub async fn oauth_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<Json<Value>, StatusCode> {
    let provider_id = query.state.unwrap_or_else(|| "google-oauth".to_string());
    let redirect_uri = default_redirect_uri();
    let provider = exchange_and_store(&state, query.code, provider_id, redirect_uri).await?;
    Ok(Json(json!({"ok": true, "provider_id": provider.id, "oauth_email": provider.oauth_email})))
}

pub async fn exchange_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<OAuthExchangeBody>,
) -> Result<Json<Value>, StatusCode> {
    require_admin(&state, &headers).await?;
    let provider_id = body.provider_id.unwrap_or_else(|| "google-oauth".to_string());
    let redirect_uri = body.redirect_uri.unwrap_or_else(default_redirect_uri);
    let provider = exchange_and_store(&state, body.code, provider_id, redirect_uri).await?;
    Ok(Json(json!({"ok": true, "provider_id": provider.id, "oauth_email": provider.oauth_email})))
}

pub async fn refresh_provider_token_if_needed(state: &AppState, provider: ProviderConfig) -> ProviderConfig {
    let Some(refresh_token) = provider.refresh_token.clone() else {
        return provider;
    };

    let should_refresh = provider
        .token_expiry
        .map(|expiry| expiry <= Utc::now().timestamp() + 120)
        .unwrap_or(false);

    if !should_refresh {
        return provider;
    }

    let Ok(token) = refresh_access_token(state, &refresh_token).await else {
        return provider;
    };

    let mut next = provider;
    next.api_key = token.access_token;
    next.token_expiry = token.expires_in.map(|seconds| Utc::now().timestamp() + seconds);
    if token.refresh_token.is_some() {
        next.refresh_token = token.refresh_token;
    }

    let saved_config = {
        let mut config = state.config.write().await;
        config.upsert_provider(next.clone());
        config.clone()
    };
    let _ = saved_config.save(&state.config_path);

    next
}

async fn exchange_and_store(
    state: &AppState,
    code: String,
    provider_id: String,
    redirect_uri: String,
) -> Result<ProviderConfig, StatusCode> {
    let token = exchange_code_for_token(state, &code, &redirect_uri).await?;
    let email = fetch_user_email(state, &token.access_token).await.unwrap_or(None);
    let expires_at = token.expires_in.map(|seconds| Utc::now().timestamp() + seconds);

    let provider = ProviderConfig {
        id: email
            .as_ref()
            .map(|value| format!("google-{}", value.replace('@', "-").replace('.', "-")))
            .unwrap_or(provider_id),
        kind: ProviderKind::Gemini,
        base_url: "https://generativelanguage.googleapis.com".to_string(),
        api_key: token.access_token,
        enabled: true,
        priority: 50,
        weight: 10,
        timeout_ms: 60000,
        models: vec!["gemini-oauth".to_string()],
        pricing: Default::default(),
        refresh_token: token.refresh_token,
        token_expiry: expires_at,
        oauth_email: email,
        oauth_provider: Some("google".to_string()),
    };

    let saved_config = {
        let mut config = state.config.write().await;
        config.upsert_provider(provider.clone());
        config.clone()
    };
    saved_config.save(&state.config_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(provider)
}

async fn exchange_code_for_token(state: &AppState, code: &str, redirect_uri: &str) -> Result<TokenResponse, StatusCode> {
    let client_id = google_client_id().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let client_secret = google_client_secret().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let response = state
        .http
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !response.status().is_success() {
        return Err(StatusCode::BAD_GATEWAY);
    }

    response.json::<TokenResponse>().await.map_err(|_| StatusCode::BAD_GATEWAY)
}

async fn refresh_access_token(state: &AppState, refresh_token: &str) -> Result<TokenResponse, StatusCode> {
    let client_id = google_client_id().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let client_secret = google_client_secret().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let response = state
        .http
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("refresh_token", refresh_token),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !response.status().is_success() {
        return Err(StatusCode::BAD_GATEWAY);
    }

    response.json::<TokenResponse>().await.map_err(|_| StatusCode::BAD_GATEWAY)
}

async fn fetch_user_email(state: &AppState, access_token: &str) -> Result<Option<String>, StatusCode> {
    let response = state
        .http
        .get(GOOGLE_USERINFO_URL)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let user = response.json::<UserInfo>().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    Ok(user.email)
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let config = state.config.read().await;
    let expected = format!("Bearer {}", config.server.admin_token);
    let got = headers.get("authorization").and_then(|v| v.to_str().ok());
    match got {
        Some(value) if value == expected => Ok(()),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

fn google_client_id() -> Result<String, std::env::VarError> {
    std::env::var("LITE_API_GOOGLE_CLIENT_ID")
}

fn google_client_secret() -> Result<String, std::env::VarError> {
    std::env::var("LITE_API_GOOGLE_CLIENT_SECRET")
}

fn default_redirect_uri() -> String {
    std::env::var("LITE_API_GOOGLE_REDIRECT_URI").unwrap_or_else(|_| "http://127.0.0.1:8082/oauth-callback".to_string())
}

fn enc(input: &str) -> String {
    input
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => vec![byte as char],
            b' ' => vec!['%','2','0'],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

use crate::{core::{diagnostics::analyze_config, state::AppState}};
use axum::{extract::State, http::{HeaderMap, StatusCode}, Json};

pub async fn get_diagnostics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<crate::core::diagnostics::DiagnosticReport>, StatusCode> {
    require_management_access(&state, &headers).await?;
    let config = state.config.read().await;
    Ok(Json(analyze_config(&config)))
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

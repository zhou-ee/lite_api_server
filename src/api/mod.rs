pub mod admin;
pub mod health;
pub mod openai;

use crate::core::state::AppState;
use axum::{routing::{delete, get, patch, post, put}, Router};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/v1/models", get(openai::models))
        .route("/v1/chat/completions", post(openai::chat_completions))
        .route("/admin/config", get(admin::get_config).put(admin::put_config))
        .route("/admin/logs", get(admin::get_logs))
        .route("/admin/stats/today", get(admin::stats_today))
        .route("/admin/stats/providers", get(admin::stats_providers))
        .route("/admin/stats/models", get(admin::stats_models))
        .route("/admin/providers", get(admin::list_providers).post(admin::upsert_provider))
        .route("/admin/providers/{id}", patch(admin::upsert_provider_by_id).delete(admin::delete_provider))
        .route("/admin/providers/{id}/healthcheck", post(admin::healthcheck_provider))
        .route("/admin/routes", get(admin::list_routes).put(admin::put_routes))
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenaiCompatible,
    Anthropic,
    Gemini,
    OpenCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPricingConfig {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub priority: i32,
    pub weight: i32,
    pub timeout_ms: u64,
    pub models: Vec<String>,
    #[serde(default)]
    pub pricing: HashMap<String, ProviderPricingConfig>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub token_expiry: Option<i64>,
    #[serde(default)]
    pub oauth_email: Option<String>,
    #[serde(default)]
    pub oauth_provider: Option<String>,
}

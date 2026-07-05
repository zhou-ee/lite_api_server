use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    OpenaiCompatible,
    Anthropic,
    Gemini,
    OpenCode,
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
}

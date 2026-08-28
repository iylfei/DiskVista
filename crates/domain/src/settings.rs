use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 1800;
pub const MAX_OUTPUT_TOKEN_LIMIT: u32 = 1_048_576;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub scan_retention: u32,
    pub enhanced_scan: bool,
    pub community_enabled: bool,
    pub protected_paths: Vec<String>,
    pub ignored_paths: Vec<String>,
    pub excluded_llm_paths: Vec<String>,
    pub labels: BTreeMap<String, String>,
    pub llm: LlmSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LlmSettings {
    pub enabled: bool,
    pub automatic: bool,
    pub metadata_consent: bool,
    pub history_reference_enabled: bool,
    pub base_url: String,
    pub model: String,
    pub format: String,
    pub token_parameter: String,
    pub max_output_tokens: u32,
    pub minimum_bytes: u64,
    pub max_requests: u32,
    pub concurrency: u32,
    pub timeout_seconds: u64,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            automatic: false,
            metadata_consent: false,
            history_reference_enabled: false,
            base_url: String::new(),
            model: String::new(),
            format: "auto".into(),
            token_parameter: "max_tokens".into(),
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
            minimum_bytes: 100 * 1024 * 1024,
            max_requests: 10,
            concurrency: 2,
            timeout_seconds: 120,
        }
    }
}

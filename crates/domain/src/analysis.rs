use super::Evidence;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisContext {
    pub scan_id: String,
    pub entry_id: i64,
    pub fingerprint: String,
    pub path: String,
    pub logical_bytes: u64,
    pub file_count: u64,
    pub modified: i64,
    pub accessed: i64,
    pub evidence: Vec<Evidence>,
    pub files: Vec<ContextFile>,
    pub truncated: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextFile {
    pub entry_id: i64,
    pub name: String,
    pub bytes: u64,
    pub is_dir: bool,
    pub modified: i64,
    pub sample_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ModelAssessment {
    pub purpose: String,
    pub source: String,
    pub consequences: String,
    pub recovery: String,
    pub recommendation: String,
    pub confidence: String,
    pub uncertainties: Vec<String>,
    pub evidence: Vec<String>,
    pub questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    pub id: String,
    pub scan_id: String,
    pub entry_id: i64,
    pub fingerprint: String,
    pub config_hash: String,
    pub created: i64,
    pub status: String,
    pub message: String,
    pub assessment: Option<ModelAssessment>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub included_content: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisProgress {
    #[serde(default)]
    pub scan_id: Option<String>,
    pub active: bool,
    pub queued: u32,
    pub finished: u32,
    pub requests: u32,
    pub max_requests: u32,
    pub message: String,
}

use super::Evidence;
use serde::{Deserialize, Serialize};

pub const ANALYSIS_FORMAT_VERSION: u32 = 3;

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
    #[serde(default)]
    pub history_references: Vec<HistoryReference>,
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
    pub deletion_advice: DeletionAdvice,
    pub reason: String,
    pub history_matches: Vec<HistoryMatch>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeletionAdvice {
    ConsiderDelete,
    Keep,
    Review,
}

impl DeletionAdvice {
    pub fn label(self) -> &'static str {
        match self {
            Self::ConsiderDelete => "可考虑删除",
            Self::Keep => "建议保留",
            Self::Review => "需要核实",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct HistoryMatch {
    pub history_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryReference {
    pub id: String,
    pub path: String,
    pub bytes: u64,
    pub recycled_at: i64,
    pub owner: Option<String>,
    pub category: Option<String>,
    pub match_basis: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisEvidence {
    pub id: String,
    pub source: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisResult {
    pub id: String,
    pub format_version: u32,
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
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default = "single_request_item")]
    pub request_item_count: u32,
    pub included_content: bool,
    #[serde(default)]
    pub evidence_details: Vec<AnalysisEvidence>,
    #[serde(default)]
    pub history_references: Vec<HistoryReference>,
}

fn single_request_item() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisSummary {
    pub analysis_id: String,
    pub entry_id: i64,
    pub created: i64,
    pub status: String,
    pub summary: String,
    pub history_match_count: usize,
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

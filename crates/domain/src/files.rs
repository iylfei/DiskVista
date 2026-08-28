use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub path: String,
    pub label: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub removable: bool,
    pub identity: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub id: i64,
    pub path: String,
    pub parent: String,
    pub name: String,
    pub is_dir: bool,
    pub logical_bytes: u64,
    pub allocated_bytes: Option<u64>,
    pub modified: i64,
    #[serde(default)]
    pub modified_ticks: i64,
    #[serde(default)]
    pub latest_change: i64,
    pub accessed: i64,
    pub created: i64,
    pub identity: Option<String>,
    pub attributes: u32,
    pub links: u32,
    pub file_count: u64,
    pub issue: Option<String>,
    pub enumerated: bool,
    pub complete: bool,
    pub has_blocked_children: bool,
    pub assessment: Assessment,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub source: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub category: String,
    pub owner: Option<String>,
    pub confidence: String,
    pub risk: String,
    pub purpose: String,
    pub consequence: String,
    pub recovery: String,
    pub recommendation: String,
    pub rule_id: Option<String>,
    pub protected_reason: Option<String>,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub publisher: String,
    pub install_location: String,
    pub source: String,
    pub last_used: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scan {
    pub id: String,
    pub root: String,
    pub started: i64,
    pub finished: Option<i64>,
    pub status: String,
    pub files: u64,
    pub directories: u64,
    pub logical_bytes: u64,
    pub allocated_bytes: u64,
    pub issues: u64,
    pub mode: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryQuery {
    pub scan_id: String,
    pub parent: Option<String>,
    pub search: Option<String>,
    pub risk: Option<String>,
    pub owner: Option<String>,
    pub suggestions: bool,
    #[serde(default)]
    pub category: Option<String>,
    pub minimum_bytes: u64,
    pub offset: u32,
    pub limit: u32,
    pub sort: Option<String>,
    #[serde(default)]
    pub analysis_status: String,
    #[serde(default)]
    pub directories_only: bool,
    #[serde(default)]
    pub uncertain_only: bool,
    #[serde(default)]
    pub issues_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPage {
    pub items: Vec<FileRecord>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    pub bytes: u64,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupItem {
    pub entry_id: i64,
    pub path: String,
    pub bytes: u64,
    pub fingerprint: String,
    pub risk: String,
    pub allowed: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupPreview {
    pub id: String,
    pub scan_id: String,
    pub created: i64,
    pub items: Vec<CleanupItem>,
    pub pending_bytes: u64,
    pub requires_extra_confirmation: bool,
    pub policy_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub batch_id: String,
    pub path: String,
    pub bytes: u64,
    pub time: i64,
    pub status: String,
    pub message: String,
    pub free_space_delta: i64,
    #[serde(default)]
    pub snapshot: Option<HistoryEntrySnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<HistoryItem>,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntrySnapshot {
    pub name: String,
    pub is_dir: bool,
    pub owner: Option<String>,
    pub category: String,
    pub rule_id: Option<String>,
}

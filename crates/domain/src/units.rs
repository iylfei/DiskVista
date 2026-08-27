use serde::{Deserialize, Serialize};

/// A read-only accounting unit, never a deletion target or an uninstall command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationUnit {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub confidence: String,
    pub logical_bytes: u64,
    pub occupied_bytes: u64,
    pub estimated: bool,
    pub file_count: u64,
    pub complete: bool,
    pub components: Vec<UnitComponent>,
    #[serde(default)]
    pub children: Vec<ApplicationUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitComponent {
    pub entry_id: i64,
    pub path: String,
    pub role: String,
    pub evidence: String,
    pub logical_bytes: u64,
    pub occupied_bytes: u64,
    pub file_count: u64,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationUnitPage {
    pub items: Vec<ApplicationUnit>,
    pub total: usize,
    pub applications: usize,
    pub uncertain: usize,
    pub occupied_bytes: u64,
    pub estimated: bool,
}

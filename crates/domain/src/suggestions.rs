use crate::FileRecord;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SuggestionQuery {
    pub scan_id: String,
    pub group: Option<String>,
    pub search: String,
    pub risk: String,
    pub sort: String,
    pub analysis_status: String,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionGroup {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub consequence: String,
    pub count: usize,
    pub occupied_bytes: u64,
    pub estimated: bool,
    pub recognized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionPage {
    pub groups: Vec<SuggestionGroup>,
    pub items: Vec<FileRecord>,
    pub total: usize,
}

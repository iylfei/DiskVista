use super::Store;
use anyhow::Result;
use cleaner_domain::HistoryItem;
use rusqlite::params;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HistoryWithScan<'a> {
    #[serde(flatten)]
    item: &'a HistoryItem,
    scan_id: &'a str,
}

impl Store {
    pub fn add_history_for_scan(&self, item: &HistoryItem, scan_id: &str) -> Result<()> {
        let data = serde_json::to_string(&HistoryWithScan { item, scan_id })?;
        self.durable_connection()?.execute(
            "INSERT INTO history VALUES(?1,?2,?3)",
            params![item.id, item.time, data],
        )?;
        Ok(())
    }
}

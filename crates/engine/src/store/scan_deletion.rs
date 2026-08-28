use super::Store;
use anyhow::{ensure, Result};
use cleaner_domain::Scan;

impl Store {
    pub fn delete_scan(&self, scan_id: &str) -> Result<Vec<Scan>> {
        let mut connection = self.durable_connection()?;
        let transaction =
            connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM scans WHERE id=?1)",
            [scan_id],
            |row| row.get(0),
        )?;
        ensure!(exists, "扫描记录不存在或已删除");
        let active: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM scans WHERE status IN ('queued','scanning','aggregating'))",
            [], |row| row.get(0),
        )?;
        ensure!(!active, "扫描运行期间不能删除记录，请先完成或取消扫描");
        let journals = {
            let mut statement =
                transaction.prepare("SELECT key,data FROM kv WHERE key LIKE 'journal:%'")?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for (key, data) in journals {
            if serde_json::from_str::<serde_json::Value>(&data)
                .is_ok_and(|value| value["scan_id"].as_str() == Some(scan_id))
            {
                transaction.execute("DELETE FROM kv WHERE key=?1", [key])?;
            }
        }
        for table in ["entries", "apps", "analyses"] {
            transaction.execute(&format!("DELETE FROM {table} WHERE scan_id=?1"), [scan_id])?;
        }
        transaction.execute(
            "DELETE FROM kv WHERE key=?1",
            [format!("llm-budget:{scan_id}")],
        )?;
        transaction.execute("DELETE FROM scans WHERE id=?1", [scan_id])?;
        let remaining = Self::scans_from(&transaction)?;
        transaction.commit()?;
        Ok(remaining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_domain::*;

    fn fixture() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("index.sqlite")).unwrap();
        std::fs::write(dir.path().join("source.txt"), "keep source").unwrap();
        for id in ["delete", "keep"] {
            store
                .save_scan(&Scan {
                    id: id.into(),
                    root: dir.path().to_string_lossy().into_owned(),
                    status: "complete".into(),
                    ..Default::default()
                })
                .unwrap();
            Store::insert_batch(
                &mut store.connection().unwrap(),
                id,
                &[FileRecord {
                    path: dir.path().join("source.txt").to_string_lossy().into_owned(),
                    ..Default::default()
                }],
            )
            .unwrap();
            let c = store.connection().unwrap();
            c.execute("INSERT INTO apps VALUES(?1,'{}')", [id]).unwrap();
            c.execute("INSERT INTO analyses VALUES(?1,?1,1,1,'{}')", [id])
                .unwrap();
            store.put(&format!("llm-budget:{id}"), &3).unwrap();
            store
                .put(&format!("journal:{id}"), &serde_json::json!({"scan_id":id}))
                .unwrap();
        }
        store.put("settings", &Settings::default()).unwrap();
        store.connection().unwrap().execute("INSERT INTO history VALUES('h',1,'{\"status\":\"recycled\",\"scanId\":\"delete\"}')", []).unwrap();
        (dir, store)
    }
    #[test]
    fn deletes_only_the_snapshot_and_its_dependents() {
        let (dir, store) = fixture();
        let remaining = store.delete_scan("delete").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "keep");
        assert!(store.scan("delete").is_err());
        assert!(store.scan("keep").is_ok());
        for table in ["entries", "apps", "analyses"] {
            let count: u32 = store
                .connection()
                .unwrap()
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE scan_id=?1"),
                    ["keep"],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1);
            let count: u32 = store
                .connection()
                .unwrap()
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE scan_id=?1"),
                    ["delete"],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }
        assert!(store.get::<u32>("llm-budget:delete").unwrap().is_none());
        assert!(store
            .get::<serde_json::Value>("journal:delete")
            .unwrap()
            .is_none());
        assert!(store
            .get::<serde_json::Value>("journal:keep")
            .unwrap()
            .is_some());
        assert!(store.get::<Settings>("settings").unwrap().is_some());
        assert_eq!(
            store
                .connection()
                .unwrap()
                .query_row("SELECT count(*) FROM history", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("source.txt")).unwrap(),
            "keep source"
        );
        assert!(store.delete_scan("delete").is_err());
    }
    #[test]
    fn active_scans_and_transaction_failure_leave_records_intact() {
        let (dir, store) = fixture();
        for status in ["queued", "scanning", "aggregating"] {
            store
                .connection()
                .unwrap()
                .execute("UPDATE scans SET status=?1 WHERE id='keep'", [status])
                .unwrap();
            assert!(store.delete_scan("delete").is_err());
            assert!(store.scan("delete").is_ok());
        }
        store.connection().unwrap().execute_batch("UPDATE scans SET status='complete'; CREATE TRIGGER fail_delete BEFORE DELETE ON scans BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(store.delete_scan("delete").is_err());
        assert!(store.scan("delete").is_ok());
        assert!(store
            .get::<serde_json::Value>("journal:delete")
            .unwrap()
            .is_some());
        assert!(store
            .by_path("delete", &dir.path().join("source.txt").to_string_lossy())
            .is_ok());
        store
            .connection()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_delete; UPDATE scans SET data='{}' WHERE id='keep';")
            .unwrap();
        assert!(store.delete_scan("delete").is_err());
        assert!(store.scan("delete").is_ok());
    }
}

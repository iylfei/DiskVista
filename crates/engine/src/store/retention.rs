use super::Store;
use anyhow::Result;
use std::collections::{HashMap, HashSet};

impl Store {
    /// Run before exposing a startup snapshot, while no worker or cleanup can use old entries.
    pub fn prune_scans(&self, keep_per_root: u32) -> Result<usize> {
        if keep_per_root == 0 {
            return Ok(0);
        }
        let keep = keep_per_root.max(2) as usize;
        let mut c = self.durable_connection()?;
        let tx = c.transaction()?;
        let scans: Vec<(String, String, String)> = {
            let mut s =
                tx.prepare("SELECT id,root,status FROM scans ORDER BY started DESC,id DESC")?;
            let rows = s
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<rusqlite::Result<_>>()?;
            rows
        };
        let mut protected = HashSet::new();
        {
            let mut s = tx.prepare("SELECT data FROM kv WHERE key LIKE 'journal:%'")?;
            for row in s.query_map([], |r| r.get::<_, String>(0))? {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&row?) {
                    if let Some(id) = value["scan_id"].as_str() {
                        protected.insert(id.to_owned());
                    }
                }
            }
        }
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut complete_roots = HashSet::new();
        let mut removed = 0;
        for (id, root, status) in scans {
            let count = counts.entry(root.clone()).or_default();
            *count += 1;
            let latest_complete = status == "complete" && complete_roots.insert(root);
            if *count <= keep
                || latest_complete
                || protected.contains(&id)
                || matches!(status.as_str(), "queued" | "scanning" | "aggregating")
            {
                continue;
            }
            for table in ["entries", "apps", "analyses"] {
                tx.execute(&format!("DELETE FROM {table} WHERE scan_id=?1"), [&id])?;
            }
            tx.execute("DELETE FROM kv WHERE key=?1", [format!("llm-budget:{id}")])?;
            tx.execute("DELETE FROM scans WHERE id=?1", [&id])?;
            removed += 1;
        }
        tx.commit()?;
        // Freed pages are reused by subsequent scans. Rebuilding the whole database here
        // would hold up bootstrap and requires temporary free space on an already full disk.
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_domain::*;

    #[test]
    fn retention_is_opt_in_and_preserves_active_baselines_and_cleanup_history() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("index.db")).unwrap();
        for i in 0..9 {
            let id = i.to_string();
            store
                .save_scan(&Scan {
                    id: id.clone(),
                    root: if i == 8 { "E:\\fixture" } else { "D:\\fixture" }.into(),
                    started: i,
                    status: if i == 1 {
                        "scanning"
                    } else if i >= 5 {
                        "failed"
                    } else {
                        "complete"
                    }
                    .into(),
                    ..Default::default()
                })
                .unwrap();
            Store::insert_batch(
                &mut store.connection().unwrap(),
                &id,
                &[FileRecord {
                    path: format!("D:\\fixture\\{i}"),
                    ..Default::default()
                }],
            )
            .unwrap();
            store.put(&format!("llm-budget:{i}"), &1).unwrap();
            let c = store.connection().unwrap();
            c.execute("INSERT INTO apps VALUES(?1,'{}')", [&id])
                .unwrap();
            c.execute("INSERT INTO analyses VALUES(?1,?1,1,1,'{}')", [&id])
                .unwrap();
        }
        store
            .put("journal:d:\\fixture", &serde_json::json!({"scan_id":"0"}))
            .unwrap();
        store.put("settings", &Settings::default()).unwrap();
        store
            .connection()
            .unwrap()
            .execute("INSERT INTO history VALUES('h',1,'{}')", [])
            .unwrap();
        assert_eq!(store.prune_scans(0).unwrap(), 0);
        assert_eq!(store.prune_scans(2).unwrap(), 3);
        let rows = |table: &str, id: &str| {
            store
                .connection()
                .unwrap()
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE scan_id=?1"),
                    [id],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap()
        };
        for id in ["0", "1", "4", "6", "7", "8"] {
            assert!(store.scan(id).is_ok());
            for table in ["entries", "apps", "analyses"] {
                assert_eq!(rows(table, id), 1);
            }
        }
        for id in ["2", "3", "5"] {
            assert!(store.scan(id).is_err());
            for table in ["entries", "apps", "analyses"] {
                assert_eq!(rows(table, id), 0);
            }
            assert!(store
                .get::<u32>(&format!("llm-budget:{id}"))
                .unwrap()
                .is_none());
        }
        assert_eq!(
            store
                .connection()
                .unwrap()
                .query_row("SELECT count(*) FROM history", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(store.settings().unwrap().scan_retention, 0);
    }
}

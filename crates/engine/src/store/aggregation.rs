use super::Store;
use anyhow::Result;
use rusqlite::params;
use std::sync::atomic::{AtomicBool, Ordering};

impl Store {
    pub fn aggregate(&self, scan: &str) -> Result<()> {
        self.aggregate_cancellable(scan, &AtomicBool::new(false))
            .map(|_| ())
    }

    pub fn aggregate_cancellable(&self, scan: &str, cancel: &AtomicBool) -> Result<bool> {
        self.aggregate_pass(scan, || cancel.load(Ordering::Relaxed), false)
    }

    pub fn propagate_protection(&self, scan: &str, cancel: &AtomicBool) -> Result<bool> {
        self.aggregate_pass(scan, || cancel.load(Ordering::Relaxed), true)
    }

    fn aggregate_pass(
        &self,
        scan: &str,
        mut cancelled: impl FnMut() -> bool,
        protection_only: bool,
    ) -> Result<bool> {
        if cancelled() {
            return Ok(false);
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction()?;
        if !protection_only {
            // A hardlink's physical bytes are charged once; logical sizes still include every name.
            tx.execute("UPDATE entries SET allocated=0 WHERE scan_id=?1 AND is_dir=0 AND identity IS NOT NULL AND id NOT IN (SELECT MIN(id) FROM entries WHERE scan_id=?1 AND is_dir=0 AND identity IS NOT NULL GROUP BY identity)", [scan])?;
        }
        let mut directories = tx.prepare("SELECT id,path_key FROM entries WHERE scan_id=?1 AND is_dir=1 ORDER BY length(path_key) DESC")?;
        let rows: Vec<(i64, String)> = directories
            .query_map([scan], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        drop(directories);
        // Aggregate in row order; size order would turn the table reads into random I/O.
        let mut totals = tx.prepare_cached("SELECT COALESCE(SUM(logical),0), CASE WHEN COUNT(*)=COUNT(allocated) THEN COALESCE(SUM(allocated),0) ELSE NULL END, COALESCE(SUM(file_count),0),COALESCE(MIN(complete),1),COALESCE(MAX(blocked OR risk='protected'),0),COALESCE(MAX(latest_change),0) FROM entries INDEXED BY entries_parent WHERE scan_id=?1 AND parent_key=?2")?;
        let mut update = tx.prepare_cached("UPDATE entries SET logical=?2,allocated=?3,file_count=?4,complete=complete AND enumerated AND ?5,blocked=?6,latest_change=MAX(latest_change,?7) WHERE id=?1")?;
        let mut protection = tx.prepare_cached("UPDATE entries SET blocked=COALESCE((SELECT MAX(child.blocked OR child.risk='protected') FROM entries child INDEXED BY entries_parent WHERE child.scan_id=?1 AND child.parent_key=?2),0) WHERE id=?3")?;
        for (id, path) in rows {
            if cancelled() {
                return Ok(false);
            }
            if protection_only {
                protection.execute(params![scan, path, id])?;
            } else {
                let (logical, allocated, files, complete, blocked, changed): (
                    u64,
                    Option<u64>,
                    u64,
                    bool,
                    bool,
                    i64,
                ) = totals.query_row(params![scan, path], |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                })?;
                update.execute(params![
                    id, logical, allocated, files, complete, blocked, changed
                ])?;
            }
        }
        drop(totals);
        drop(update);
        drop(protection);
        if cancelled() {
            return Ok(false);
        }
        tx.commit()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_domain::{Assessment, FileRecord};

    #[test]
    fn protection_pass_matches_full_aggregation_without_changing_sizes_or_hardlinks() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("aggregation.sqlite")).unwrap();
        let records: Vec<_> = [
            ("D:\\root", "D:", true, None),
            ("D:\\root\\nested", "D:\\root", true, None),
            (
                "D:\\root\\nested\\a",
                "D:\\root\\nested",
                false,
                Some("same"),
            ),
            ("D:\\root\\b", "D:\\root", false, Some("same")),
        ]
        .into_iter()
        .map(|(path, parent, is_dir, identity)| FileRecord {
            path: path.into(),
            parent: parent.into(),
            is_dir,
            identity: identity.map(str::to_owned),
            logical_bytes: if is_dir { 0 } else { 10 },
            allocated_bytes: Some(if is_dir { 0 } else { 4096 }),
            file_count: u64::from(!is_dir),
            complete: true,
            enumerated: true,
            assessment: Assessment {
                risk: "review".into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .collect();
        for scan in ["optimized", "reference"] {
            Store::insert_batch(&mut store.connection().unwrap(), scan, &records).unwrap();
            store.aggregate(scan).unwrap();
            let leaf = store.by_path(scan, "D:\\root\\nested\\a").unwrap();
            store
                .update_assessment(
                    leaf.id,
                    &Assessment {
                        risk: "protected".into(),
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        store.aggregate("reference").unwrap();
        assert!(store
            .propagate_protection("optimized", &AtomicBool::new(false))
            .unwrap());
        Store::insert_batch(&mut store.connection().unwrap(), "cancelled", &records).unwrap();
        let mut checks = 0;
        assert!(!store
            .aggregate_pass(
                "cancelled",
                || {
                    checks += 1;
                    checks == 3 // Cancel after updating the deepest directory, before updating its parent.
                },
                false
            )
            .unwrap());
        assert_eq!(checks, 3);
        for record in records {
            let cancelled = store.by_path("cancelled", &record.path).unwrap();
            assert_eq!(cancelled.logical_bytes, record.logical_bytes);
            assert_eq!(cancelled.allocated_bytes, record.allocated_bytes);
            assert_eq!(cancelled.file_count, record.file_count);
            let actual = store.by_path("optimized", &record.path).unwrap();
            let expected = store.by_path("reference", &record.path).unwrap();
            assert_eq!(
                (
                    actual.logical_bytes,
                    actual.allocated_bytes,
                    actual.file_count,
                    actual.complete,
                    actual.has_blocked_children
                ),
                (
                    expected.logical_bytes,
                    expected.allocated_bytes,
                    expected.file_count,
                    expected.complete,
                    expected.has_blocked_children
                )
            );
        }
        let before = store.by_path("optimized", "D:\\root").unwrap();
        assert!(!store
            .aggregate_cancellable("optimized", &AtomicBool::new(true))
            .unwrap());
        assert_eq!(
            before.logical_bytes,
            store
                .by_path("optimized", "D:\\root")
                .unwrap()
                .logical_bytes
        );
    }
}

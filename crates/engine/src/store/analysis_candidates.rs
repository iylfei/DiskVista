use super::{decode, Store, FIELDS};
use anyhow::{ensure, Result};
use cleaner_domain::FileRecord;
use cleaner_platform::normalize;
use rusqlite::params;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

impl Store {
    pub fn analysis_candidate_page(
        &self,
        scan: &str,
        minimum_bytes: u64,
        after: Option<(u64, i64)>,
        cancel: Arc<AtomicBool>,
    ) -> Result<Vec<FileRecord>> {
        ensure!(!cancel.load(Ordering::Relaxed), "分析已取消");
        let root = self.require_finished(scan)?.root;
        let connection = self.connection()?;
        let flag = Arc::clone(&cancel);
        connection.progress_handler(1000, Some(move || flag.load(Ordering::Relaxed)));
        let fetch = |bound: &str, size: u64, id: i64, limit: usize| -> Result<Vec<FileRecord>> {
            let mut statement = connection.prepare(&format!("SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND is_dir=0 AND logical>?2 AND complete=1 AND blocked=0 AND path_key<>?3
                AND {bound} ORDER BY logical DESC,id LIMIT ?6"))?;
            let result = statement
                .query_map(
                    params![
                        scan,
                        minimum_bytes.min(i64::MAX as u64) as i64,
                        normalize(&root),
                        size.min(i64::MAX as u64) as i64,
                        id,
                        limit as i64
                    ],
                    decode,
                )?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(result)
        };
        // Seek inside equal-size runs first, rather than rescanning all earlier ties.
        let result = (|| -> Result<Vec<FileRecord>> {
            Ok(if let Some((size, id)) = after {
                let mut rows = fetch("logical=?4 AND id>?5", size, id, 200)?;
                if rows.len() < 200 {
                    rows.extend(fetch("logical<?4 AND id>?5", size, 0, 200 - rows.len())?);
                }
                rows
            } else {
                fetch("logical<=?4 AND id>?5", i64::MAX as u64, 0, 200)?
            })
        })();
        ensure!(!cancel.load(Ordering::Relaxed), "分析已取消");
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_domain::Scan;
    #[test]
    fn pages_keep_equal_size_ties_without_duplicates_and_cancel_before_reading() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("candidates.sqlite")).unwrap();
        store
            .save_scan(&Scan {
                id: "s".into(),
                root: r"D:\fixture".into(),
                status: "complete".into(),
                ..Default::default()
            })
            .unwrap();
        let files: Vec<_> = (1..=701)
            .map(|i| FileRecord {
                path: format!(r"D:\fixture\{i}"),
                logical_bytes: if i <= 600 { 200 } else { 100 },
                complete: true,
                ..Default::default()
            })
            .collect();
        Store::insert_batch(&mut store.connection().unwrap(), "s", &files).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut ids = Vec::new();
        let mut after = None;
        loop {
            let page = store
                .analysis_candidate_page("s", 0, after, cancel.clone())
                .unwrap();
            assert!(page.len() <= 200);
            if page.is_empty() {
                break;
            }
            after = page.last().map(|f| (f.logical_bytes, f.id));
            ids.extend(page.into_iter().map(|f| f.id));
        }
        assert_eq!(ids, (1..=701).collect::<Vec<_>>());
        cancel.store(true, Ordering::Relaxed);
        assert!(store
            .analysis_candidate_page("s", 0, None, cancel)
            .unwrap_err()
            .to_string()
            .contains("取消"));
    }
}

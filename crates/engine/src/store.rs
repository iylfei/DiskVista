use anyhow::{anyhow, Result};
use cleaner_domain::*;
use cleaner_platform::normalize;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
mod aggregation;
mod analysis_usage;
mod entry_data;
mod history_paging;
mod query;
mod recycled_history;
mod retention;
mod revision;
pub use revision::RevisionObserver;
mod scan_deletion;
mod subtrees;

#[derive(Clone)]
pub struct Store {
    pub path: PathBuf,
}
pub(crate) const FIELDS: &str = "id,data,logical,allocated,file_count,complete,blocked,latest_change,enumerated,issue,assessment";
pub(crate) fn decode(row: &Row<'_>) -> rusqlite::Result<FileRecord> {
    let data: String = row.get(1)?;
    let mut f: FileRecord = serde_json::from_str(&data).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    f.id = row.get(0)?;
    f.logical_bytes = row.get(2)?;
    f.allocated_bytes = row.get(3)?;
    f.file_count = row.get(4)?;
    f.complete = row.get(5)?;
    f.has_blocked_children = row.get(6)?;
    f.latest_change = row.get(7)?;
    f.enumerated = row.get(8)?;
    f.issue = row.get(9)?;
    let assessment: String = row.get(10)?;
    f.assessment = serde_json::from_str(&assessment).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(f)
}
impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let s = Self {
            path: path.as_ref().to_owned(),
        };
        s.connection()?.execute_batch("PRAGMA journal_mode=WAL;
          CREATE TABLE IF NOT EXISTS scans(id TEXT PRIMARY KEY, root TEXT NOT NULL, started INTEGER NOT NULL, status TEXT NOT NULL, data TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS entries(id INTEGER PRIMARY KEY, scan_id TEXT NOT NULL,path_key TEXT NOT NULL,parent_key TEXT NOT NULL,is_dir INTEGER NOT NULL,identity TEXT,logical INTEGER NOT NULL,allocated INTEGER,file_count INTEGER NOT NULL,complete INTEGER NOT NULL,blocked INTEGER NOT NULL,latest_change INTEGER NOT NULL,enumerated INTEGER NOT NULL,issue TEXT,risk TEXT NOT NULL,owner TEXT,rule_id TEXT,data TEXT NOT NULL,assessment TEXT NOT NULL,UNIQUE(scan_id,path_key));
          CREATE INDEX IF NOT EXISTS entries_parent ON entries(scan_id,parent_key);
          CREATE INDEX IF NOT EXISTS entries_parent_size ON entries(scan_id,parent_key,logical DESC,id);
          CREATE INDEX IF NOT EXISTS entries_executable_parents ON entries(scan_id,parent_key) WHERE is_dir=0 AND path_key LIKE '%.exe';
          CREATE INDEX IF NOT EXISTS entries_pending ON entries(scan_id,is_dir,enumerated);
          CREATE INDEX IF NOT EXISTS entries_size ON entries(scan_id,logical DESC);
          CREATE INDEX IF NOT EXISTS entries_identity ON entries(scan_id,identity);
          CREATE INDEX IF NOT EXISTS entries_cursor ON entries(scan_id,id);
          CREATE INDEX IF NOT EXISTS entries_rule_candidates ON entries(scan_id,id) WHERE rule_id IS NOT NULL;
          CREATE INDEX IF NOT EXISTS entries_large_candidates ON entries(scan_id,logical) WHERE is_dir=0 AND rule_id IS NULL AND logical>=104857600;
          CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY,data TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS apps(scan_id TEXT NOT NULL,data TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS apps_scan ON apps(scan_id);
          CREATE INDEX IF NOT EXISTS scans_started ON scans(started DESC);
          CREATE TABLE IF NOT EXISTS analyses(id TEXT PRIMARY KEY,scan_id TEXT NOT NULL,entry_id INTEGER NOT NULL,created INTEGER NOT NULL,data TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS analyses_entry ON analyses(scan_id,entry_id,created DESC);
          CREATE TABLE IF NOT EXISTS history(id TEXT PRIMARY KEY,time INTEGER NOT NULL,data TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS history_time ON history(time DESC,id);
          CREATE INDEX IF NOT EXISTS history_recycled_scan ON history(json_extract(data,'$.scanId'),time) WHERE json_extract(data,'$.status')='recycled';")?;
        s.discard_legacy_analyses()?;
        Ok(s)
    }
    pub fn connection(&self) -> Result<Connection> {
        let c = Connection::open(&self.path)?;
        c.busy_timeout(Duration::from_secs(15))?;
        c.pragma_update(None, "synchronous", "NORMAL")?;
        Ok(c)
    }
    fn durable_connection(&self) -> Result<Connection> {
        let c = self.connection()?;
        c.pragma_update(None, "synchronous", "FULL")?;
        Ok(c)
    }
    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.durable_connection()?.execute(
            "INSERT OR REPLACE INTO kv VALUES(?1,?2)",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let s: Option<String> = self
            .connection()?
            .query_row("SELECT data FROM kv WHERE key=?1", [key], |r| r.get(0))
            .optional()?;
        s.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
    }
    /// Persist reservations monotonically even when concurrent requests finish out of order.
    pub fn put_counter_max(&self, key: &str, value: u32) -> Result<()> {
        self.durable_connection()?.execute(
            "INSERT INTO kv(key,data) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET data=CAST(MAX(CAST(kv.data AS INTEGER),CAST(excluded.data AS INTEGER)) AS TEXT)",
            params![key, value.to_string()],
        )?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings> {
        Ok(self.get("settings")?.unwrap_or_default())
    }
    pub fn save_scan(&self, scan: &Scan) -> Result<()> {
        self.connection()?.execute(
            "INSERT OR REPLACE INTO scans VALUES(?1,?2,?3,?4,?5)",
            params![
                scan.id,
                normalize(&scan.root),
                scan.started,
                scan.status,
                serde_json::to_string(scan)?
            ],
        )?;
        Ok(())
    }
    pub fn scans(&self) -> Result<Vec<Scan>> {
        let c = self.connection()?;
        Self::scans_from(&c)
    }
    fn scans_from(c: &Connection) -> Result<Vec<Scan>> {
        let mut s = c.prepare("SELECT data FROM scans ORDER BY started DESC LIMIT 30")?;
        let result = s
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|v| Ok(serde_json::from_str(&v?)?))
            .collect();
        result
    }
    pub fn scan(&self, id: &str) -> Result<Scan> {
        let c = self.connection()?;
        let json: String = c.query_row("SELECT data FROM scans WHERE id=?1", [id], |r| r.get(0))?;
        Ok(serde_json::from_str(&json)?)
    }
    pub fn save_apps(&self, id: &str, apps: &[InstalledApp]) -> Result<()> {
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        tx.execute("DELETE FROM apps WHERE scan_id=?1", [id])?;
        for a in apps {
            tx.execute(
                "INSERT INTO apps VALUES(?1,?2)",
                params![id, serde_json::to_string(a)?],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn apps(&self, id: &str) -> Result<Vec<InstalledApp>> {
        let c = self.connection()?;
        let mut s = c.prepare("SELECT data FROM apps WHERE scan_id=?1")?;
        let result = s
            .query_map([id], |r| r.get::<_, String>(0))?
            .map(|v| Ok(serde_json::from_str(&v?)?))
            .collect();
        result
    }
    pub fn insert_batch(c: &mut Connection, scan_id: &str, files: &[FileRecord]) -> Result<()> {
        Self::insert_scan_batch(c, scan_id, files, &[])
    }
    pub(crate) fn insert_scan_batch(
        c: &mut Connection,
        scan_id: &str,
        files: &[FileRecord],
        finished: &[(i64, Option<String>)],
    ) -> Result<()> {
        let tx = c.transaction()?;
        {
            let mut stmt=tx.prepare_cached("INSERT OR REPLACE INTO entries(scan_id,path_key,parent_key,is_dir,identity,logical,allocated,file_count,complete,blocked,latest_change,enumerated,issue,risk,owner,rule_id,data,assessment) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)")?;
            for f in files {
                stmt.execute(params![
                    scan_id,
                    normalize(&f.path),
                    normalize(&f.parent),
                    f.is_dir,
                    f.identity,
                    f.logical_bytes,
                    f.allocated_bytes,
                    f.file_count,
                    f.complete,
                    f.has_blocked_children,
                    f.latest_change.max(f.modified),
                    f.enumerated,
                    f.issue,
                    f.assessment.risk,
                    f.assessment.owner,
                    f.assessment.rule_id,
                    serde_json::to_string(&entry_data::Metadata(f))?,
                    serde_json::to_string(&f.assessment)?
                ])?;
            }
        }
        {
            let mut stmt = tx.prepare_cached(
                "UPDATE entries SET enumerated=1,complete=?3,issue=?4 WHERE scan_id=?1 AND id=?2",
            )?;
            for (id, error) in finished {
                stmt.execute(params![scan_id, id, error.is_none(), error])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn entry(&self, scan: &str, id: i64) -> Result<FileRecord> {
        Ok(self.connection()?.query_row(
            &format!("SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND id=?2"),
            params![scan, id],
            decode,
        )?)
    }
    pub fn by_path(&self, scan: &str, path: &str) -> Result<FileRecord> {
        Ok(self.connection()?.query_row(
            &format!("SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND path_key=?2"),
            params![scan, normalize(path)],
            decode,
        )?)
    }
    pub fn pending(&self, scan: &str, limit: usize) -> Result<Vec<FileRecord>> {
        let c = self.connection()?;
        Self::pending_from(&c, scan, limit)
    }
    pub(crate) fn pending_from(
        c: &Connection,
        scan: &str,
        limit: usize,
    ) -> Result<Vec<FileRecord>> {
        let mut stmt = c.prepare(&format!(
            "SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND is_dir=1 AND enumerated=0 LIMIT ?2"
        ))?;
        let result = stmt
            .query_map(params![scan, limit as i64], decode)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(result)
    }
    /// Internal sequential processing must not repeatedly sort/count the entire index.
    pub fn page_after(
        &self,
        scan: &str,
        after: i64,
        directories_only: bool,
    ) -> Result<Vec<FileRecord>> {
        let c = self.connection()?;
        let mut s = c.prepare(&format!(
            "SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND id>?2 {} ORDER BY id LIMIT 512",
            if directories_only { "AND is_dir=1" } else { "" }
        ))?;
        let rows = s
            .query_map(params![scan, after], decode)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
    pub fn finish_directory(&self, scan: &str, id: i64, error: Option<String>) -> Result<()> {
        self.connection()?.execute(
            "UPDATE entries SET enumerated=1,complete=?3,issue=?4 WHERE scan_id=?1 AND id=?2",
            params![scan, id, error.is_none(), error],
        )?;
        Ok(())
    }
    pub fn update_assessment(&self, id: i64, a: &Assessment) -> Result<()> {
        self.connection()?.execute(
            "UPDATE entries SET risk=?2,owner=?3,rule_id=?4,assessment=?5 WHERE id=?1",
            params![id, a.risk, a.owner, a.rule_id, serde_json::to_string(a)?],
        )?;
        Ok(())
    }
    pub fn update_assessments(&self, updates: &[(i64, Assessment)]) -> Result<()> {
        let mut c = self.connection()?;
        let tx = c.transaction()?;
        {
            let mut s = tx.prepare_cached(
                "UPDATE entries SET risk=?2,owner=?3,rule_id=?4,assessment=?5 WHERE id=?1",
            )?;
            for (id, a) in updates {
                s.execute(params![
                    id,
                    a.risk,
                    a.owner,
                    a.rule_id,
                    serde_json::to_string(a)?
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn stats(&self, scan: &str) -> Result<(u64, u64, u64)> {
        Ok(self.connection()?.query_row("SELECT COALESCE(SUM(is_dir=0),0),COALESCE(SUM(is_dir=1),0),COALESCE(SUM(issue IS NOT NULL),0) FROM entries WHERE scan_id=?1",[scan],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)
    }
    pub fn descendants(&self, scan: &str, path: &str) -> Result<Vec<FileRecord>> {
        self.descendants_bounded(scan, path, usize::MAX)
    }
    pub fn descendants_bounded(
        &self,
        scan: &str,
        path: &str,
        maximum: usize,
    ) -> Result<Vec<FileRecord>> {
        let c = self.connection()?;
        let mut s=c.prepare(&format!("WITH RECURSIVE tree(id,path_key) AS (SELECT id,path_key FROM entries WHERE scan_id=?1 AND path_key=?2 UNION ALL SELECT e.id,e.path_key FROM entries e JOIN tree t ON e.parent_key=t.path_key WHERE e.scan_id=?1) SELECT {FIELDS} FROM entries WHERE id IN (SELECT id FROM tree) ORDER BY path_key LIMIT ?3"))?;
        let result = s
            .query_map(
                params![
                    scan,
                    normalize(path),
                    maximum.saturating_add(1).min(i64::MAX as usize) as i64
                ],
                decode,
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        anyhow::ensure!(
            result.len() <= maximum,
            "单个目标超过十万项，请选择更小的目录后重试"
        );
        Ok(result)
    }
    pub fn add_history(&self, h: &HistoryItem) -> Result<()> {
        self.durable_connection()?.execute(
            "INSERT INTO history VALUES(?1,?2,?3)",
            params![h.id, h.time, serde_json::to_string(h)?],
        )?;
        Ok(())
    }
    pub fn recent_recycled_history(
        &self,
        since: i64,
        until: i64,
        limit: usize,
    ) -> Result<Vec<HistoryItem>> {
        let c = self.connection()?;
        let mut statement = c.prepare("SELECT data FROM history WHERE time>=?1 AND time<=?2 AND json_extract(data,'$.status')='recycled' ORDER BY time DESC,id DESC LIMIT ?3")?;
        let result = statement
            .query_map(params![since, until, limit.min(200) as i64], |row| {
                row.get::<_, String>(0)
            })?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect();
        result
    }

    pub fn analysis_candidate_pool(
        &self,
        scan: &str,
        minimum_bytes: u64,
    ) -> Result<Vec<FileRecord>> {
        let root = self.require_finished(scan)?.root;
        let c = self.connection()?;
        let mut statement = c.prepare(&format!("SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND is_dir=0 AND logical>?2 AND complete=1 AND blocked=0 AND path_key<>?3 ORDER BY logical DESC,id"))?;
        let result = statement
            .query_map(
                params![
                    scan,
                    minimum_bytes.min(i64::MAX as u64) as i64,
                    normalize(&root)
                ],
                decode,
            )?
            .collect::<rusqlite::Result<_>>()?;
        Ok(result)
    }
    pub fn save_analysis(&self, a: &AnalysisResult) -> Result<()> {
        self.save_analyses(std::slice::from_ref(a))
    }
    pub fn save_analyses(&self, results: &[AnalysisResult]) -> Result<()> {
        if results.is_empty() {
            return Ok(());
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction()?;
        for a in results {
            tx.execute(
                "INSERT OR REPLACE INTO analyses VALUES(?1,?2,?3,?4,?5)",
                params![
                    a.id,
                    a.scan_id,
                    a.entry_id,
                    a.created,
                    serde_json::to_string(a)?
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn analyses(&self, scan: &str, id: i64) -> Result<Vec<AnalysisResult>> {
        let c = self.connection()?;
        let mut s=c.prepare("SELECT data FROM analyses WHERE scan_id=?1 AND entry_id=?2 ORDER BY created DESC LIMIT 10")?;
        let result = s
            .query_map(params![scan, id], |r| r.get::<_, String>(0))?
            .map(|v| Ok(serde_json::from_str(&v?)?))
            .collect();
        result
    }
    pub fn analyses_for_entries(&self, scan: &str, ids: &[i64]) -> Result<Vec<AnalysisResult>> {
        anyhow::ensure!(ids.len() <= 200, "每次最多查询 200 项 AI 摘要");
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let c = self.connection()?;
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!("SELECT data FROM (SELECT entry_id,created,id,data,ROW_NUMBER() OVER(PARTITION BY entry_id ORDER BY created DESC,id DESC) AS rank FROM analyses WHERE scan_id=? AND entry_id IN ({placeholders})) WHERE rank<=10 ORDER BY entry_id,created DESC,id DESC");
        let mut args: Vec<rusqlite::types::Value> = vec![scan.to_owned().into()];
        args.extend(ids.iter().copied().map(Into::into));
        let mut statement = c.prepare(&sql)?;
        let result = statement
            .query_map(rusqlite::params_from_iter(args), |row| {
                row.get::<_, String>(0)
            })?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect();
        result
    }
    pub fn analysis_entry_ids(&self, scan: &str) -> Result<Vec<i64>> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT DISTINCT entry_id FROM analyses WHERE scan_id=? ORDER BY entry_id")?;
        let ids = statement
            .query_map([scan], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ids)
    }
    pub fn recover_interrupted(&self) -> Result<()> {
        let c = self.connection()?;
        let mut statement = c.prepare(
            "SELECT data FROM scans WHERE status IN ('scanning','aggregating','queued')",
        )?;
        let scans = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for json in scans {
            let mut s: Scan = serde_json::from_str(&json)?;
            if ["scanning", "aggregating", "queued"].contains(&s.status.as_str()) {
                s.status = "interrupted".into();
                s.finished = Some(chrono::Utc::now().timestamp());
                s.message = "上次扫描未正常结束，索引不完整，请重新扫描".into();
                self.save_scan(&s)?;
            }
        }
        Ok(())
    }
    pub fn require_finished(&self, id: &str) -> Result<Scan> {
        let s = self.scan(id)?;
        if s.status != "complete" {
            return Err(anyhow!("扫描未完成，不能据此执行清理"));
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connection_durability_does_not_leak_between_index_and_authoritative_writes() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("index.db")).unwrap();
        let mode = |c: Connection| {
            c.pragma_query_value(None, "synchronous", |r| r.get::<_, i64>(0))
                .unwrap()
        };
        assert_eq!(mode(store.connection().unwrap()), 1);
        assert_eq!(mode(store.durable_connection().unwrap()), 2);
        store
            .put(
                "settings",
                &Settings {
                    scan_retention: 5,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(mode(store.connection().unwrap()), 1);
        assert_eq!(
            Store::open(&store.path)
                .unwrap()
                .settings()
                .unwrap()
                .scan_retention,
            5
        );
    }
    #[test]
    fn issue_filter_returns_only_recorded_failures_before_pagination() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("issues.sqlite")).unwrap();
        let records: Vec<_> = [
            ("a", None),
            ("b", Some("访问被拒绝")),
            ("c", Some("设备断开")),
        ]
        .into_iter()
        .map(|(name, issue)| FileRecord {
            path: format!("D:\\fixture\\{name}"),
            issue: issue.map(String::from),
            complete: issue.is_none(),
            ..Default::default()
        })
        .collect();
        Store::insert_batch(&mut store.connection().unwrap(), "s", &records).unwrap();
        let query = EntryQuery {
            scan_id: "s".into(),
            issues_only: true,
            sort: Some("name".into()),
            offset: 1,
            limit: 1,
            ..Default::default()
        };
        let result = store.query(&query).unwrap();
        assert_eq!(result.total, 2);
        assert_eq!(result.items[0].path, "D:\\fixture\\c");
        assert_eq!(result.items[0].issue.as_deref(), Some("设备断开"));
    }
    #[test]
    fn directory_query_keeps_empty_files_and_respects_size_threshold() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::open(d.path().join("size-filter.sqlite")).unwrap();
        let records: Vec<_> = [0, 128, 256]
            .into_iter()
            .map(|size| FileRecord {
                path: format!("D:\\fixture\\{size}.txt"),
                parent: "D:\\fixture".into(),
                logical_bytes: size,
                ..Default::default()
            })
            .collect();
        Store::insert_batch(&mut store.connection().unwrap(), "s", &records).unwrap();
        for (minimum_bytes, expected) in [(0, 3), (128, 2), (256, 1)] {
            let page = store
                .query(&EntryQuery {
                    scan_id: "s".into(),
                    parent: Some("D:\\fixture".into()),
                    minimum_bytes,
                    limit: 100,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(page.total, expected);
            assert!(page
                .items
                .iter()
                .all(|file| file.logical_bytes >= minimum_bytes));
        }
    }
    #[test]
    fn persisted_budget_never_moves_backwards() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::open(d.path().join("budget.sqlite")).unwrap();
        std::thread::scope(|scope| {
            for n in (1..=40).rev() {
                let s = &store;
                scope.spawn(move || s.put_counter_max("budget", n).unwrap());
            }
        });
        assert_eq!(store.get::<u32>("budget").unwrap(), Some(40));
    }
    #[test]
    fn paging_and_identity_accounting() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::open(d.path().join("index.db")).unwrap();
        let mut c = s.connection().unwrap();
        let mut root = FileRecord {
            path: "D:\\fixture".into(),
            is_dir: true,
            enumerated: true,
            complete: true,
            ..Default::default()
        };
        root.assessment.risk = "review".into();
        let mut a = FileRecord {
            path: "D:\\fixture\\a".into(),
            parent: root.path.clone(),
            logical_bytes: 100,
            allocated_bytes: Some(128),
            file_count: 1,
            complete: true,
            identity: Some("vol:1".into()),
            ..Default::default()
        };
        a.assessment.risk = "review".into();
        let mut b = a.clone();
        b.path = "D:\\fixture\\b".into();
        Store::insert_batch(&mut c, "s", &[root, a, b]).unwrap();
        s.aggregate("s").unwrap();
        let root = s.by_path("s", "D:\\fixture").unwrap();
        assert_eq!(root.logical_bytes, 200);
        assert_eq!(root.allocated_bytes, Some(128));
        let p = s
            .query(&EntryQuery {
                scan_id: "s".into(),
                parent: Some(root.path),
                limit: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(p.total, 2);
        assert_eq!(p.items.len(), 1);
    }

    #[test]
    fn category_and_unknown_filters_are_distinct() {
        let d = tempfile::tempdir().unwrap();
        let store = Store::open(d.path().join("groups.sqlite")).unwrap();
        let unknown = FileRecord {
            path: "D:\\fixture\\unknown".into(),
            logical_bytes: 104_857_600,
            complete: true,
            assessment: Assessment {
                risk: "review".into(),
                category: "unknown".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut cache = unknown.clone();
        cache.path = "D:\\fixture\\cache".into();
        cache.assessment.owner = Some("Example".into());
        cache.assessment.category = "cache".into();
        Store::insert_batch(&mut store.connection().unwrap(), "s", &[unknown, cache]).unwrap();
        for risk in ["unknown", "known_review"] {
            assert_eq!(
                store
                    .query(&EntryQuery {
                        scan_id: "s".into(),
                        risk: Some(risk.into()),
                        suggestions: true,
                        limit: 100,
                        ..Default::default()
                    })
                    .unwrap()
                    .total,
                1
            );
        }
        assert_eq!(
            store
                .query(&EntryQuery {
                    scan_id: "s".into(),
                    category: Some("cache".into()),
                    limit: 100,
                    ..Default::default()
                })
                .unwrap()
                .items[0]
                .assessment
                .owner
                .as_deref(),
            Some("Example")
        );
    }

    #[test]
    fn suggestions_hide_effectively_protected_targets_before_pagination() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("suggestions.sqlite")).unwrap();
        store
            .save_scan(&Scan {
                id: "s".into(),
                root: "D:\\Fixture\\".into(),
                ..Default::default()
            })
            .unwrap();
        let records: Vec<_> = [
            ("D:\\Fixture", true, false, "review"),
            ("D:\\Fixture\\protected", true, false, "protected"),
            ("D:\\Fixture\\incomplete", false, false, "review"),
            ("D:\\Fixture\\blocked", true, true, "review"),
            ("D:\\Fixture\\a", true, false, "review"),
            ("D:\\Fixture\\b", true, false, "low"),
        ]
        .into_iter()
        .map(|(path, complete, blocked, risk)| FileRecord {
            path: path.into(),
            logical_bytes: 200_000_000,
            complete,
            has_blocked_children: blocked,
            assessment: Assessment {
                risk: risk.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .collect();
        Store::insert_batch(&mut store.connection().unwrap(), "s", &records).unwrap();
        let mut q = EntryQuery {
            scan_id: "s".into(),
            suggestions: true,
            limit: 1,
            ..Default::default()
        };
        let first = store.query(&q).unwrap();
        assert_eq!(first.total, 2);
        assert_eq!(first.items[0].path, "D:\\Fixture\\a");
        q.offset = 1;
        assert_eq!(store.query(&q).unwrap().items[0].path, "D:\\Fixture\\b");
        q.offset = 0;
        q.risk = Some("protected".into());
        q.limit = 100;
        let protected = store.query(&q).unwrap();
        assert_eq!(protected.total, 4);
        assert!(protected.items.iter().any(|f| f.path == "D:\\Fixture"));
        q.risk = None;
        q.suggestions = false;
        assert_eq!(store.query(&q).unwrap().total, 6);
    }
}

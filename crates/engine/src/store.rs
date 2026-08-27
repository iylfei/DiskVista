use anyhow::{anyhow, Result};
use cleaner_domain::*;
use cleaner_platform::normalize;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

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
        s.connection()?.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
          CREATE TABLE IF NOT EXISTS scans(id TEXT PRIMARY KEY, root TEXT NOT NULL, started INTEGER NOT NULL, status TEXT NOT NULL, data TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS entries(id INTEGER PRIMARY KEY, scan_id TEXT NOT NULL,path_key TEXT NOT NULL,parent_key TEXT NOT NULL,is_dir INTEGER NOT NULL,identity TEXT,logical INTEGER NOT NULL,allocated INTEGER,file_count INTEGER NOT NULL,complete INTEGER NOT NULL,blocked INTEGER NOT NULL,latest_change INTEGER NOT NULL,enumerated INTEGER NOT NULL,issue TEXT,risk TEXT NOT NULL,owner TEXT,rule_id TEXT,data TEXT NOT NULL,assessment TEXT NOT NULL,UNIQUE(scan_id,path_key));
          CREATE INDEX IF NOT EXISTS entries_parent ON entries(scan_id,parent_key);
          CREATE INDEX IF NOT EXISTS entries_pending ON entries(scan_id,is_dir,enumerated);
          CREATE INDEX IF NOT EXISTS entries_size ON entries(scan_id,logical DESC);
          CREATE INDEX IF NOT EXISTS entries_identity ON entries(scan_id,identity);
          CREATE INDEX IF NOT EXISTS entries_cursor ON entries(scan_id,id);
          CREATE TABLE IF NOT EXISTS kv(key TEXT PRIMARY KEY,data TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS apps(scan_id TEXT NOT NULL,data TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS analyses(id TEXT PRIMARY KEY,scan_id TEXT NOT NULL,entry_id INTEGER NOT NULL,created INTEGER NOT NULL,data TEXT NOT NULL);
          CREATE INDEX IF NOT EXISTS analyses_entry ON analyses(scan_id,entry_id,created DESC);
          CREATE TABLE IF NOT EXISTS history(id TEXT PRIMARY KEY,time INTEGER NOT NULL,data TEXT NOT NULL);")?;
        Ok(s)
    }
    pub fn connection(&self) -> Result<Connection> {
        let c = Connection::open(&self.path)?;
        c.busy_timeout(Duration::from_secs(15))?;
        Ok(c)
    }
    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.connection()?.execute(
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
        self.connection()?.execute(
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
                    serde_json::to_string(f)?,
                    serde_json::to_string(&f.assessment)?
                ])?;
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
    pub fn query(&self, q: &EntryQuery) -> Result<EntryPage> {
        let c = self.connection()?;
        let scan_root: Option<String> = c
            .query_row("SELECT root FROM scans WHERE id=?1", [&q.scan_id], |r| {
                r.get(0)
            })
            .optional()?;
        let scan_root = scan_root.map(|root| normalize(&root)).unwrap_or_default();
        let mut cond = String::from("scan_id=?");
        let mut args: Vec<rusqlite::types::Value> = vec![q.scan_id.clone().into()];
        if q.directories_only {
            cond.push_str(" AND is_dir=1");
        }
        if q.issues_only {
            cond.push_str(" AND issue IS NOT NULL");
        }
        if q.uncertain_only {
            cond.push_str(" AND risk='review' AND rule_id IS NULL AND json_extract(assessment,'$.confidence')='low'");
        }
        if let Some(parent) = &q.parent {
            cond.push_str(" AND parent_key=?");
            args.push(normalize(parent).into());
        }
        if let Some(search) = q.search.as_ref().filter(|s| !s.is_empty()) {
            cond.push_str(" AND instr(path_key,?)>0");
            args.push(normalize(search).into());
        }
        if let Some(risk) = q.risk.as_ref().filter(|s| !s.is_empty()) {
            match risk.as_str() {
                "protected" => {
                    cond.push_str(
                        " AND (risk='protected' OR complete=0 OR blocked=1 OR path_key=?)",
                    );
                    args.push(scan_root.clone().into());
                }
                "unknown" => {
                    cond.push_str(" AND risk='review' AND owner IS NULL AND rule_id IS NULL")
                }
                "known" => cond.push_str(" AND (owner IS NOT NULL OR rule_id IS NOT NULL)"),
                "known_review" => cond
                    .push_str(" AND risk='review' AND (owner IS NOT NULL OR rule_id IS NOT NULL)"),
                _ => {
                    cond.push_str(" AND risk=?");
                    args.push(risk.clone().into());
                }
            }
        }
        if let Some(category) = &q.category {
            cond.push_str(" AND json_extract(assessment,'$.category')=?");
            args.push(category.clone().into());
        }
        if let Some(owner) = &q.owner {
            cond.push_str(" AND COALESCE(owner,'未知')=?");
            args.push(owner.clone().into());
        }
        // All indexed sizes are non-negative. A redundant >= 0 range can make
        // SQLite choose a whole-snapshot size index over the directory index.
        if q.minimum_bytes > 0 {
            cond.push_str(" AND logical>=?");
            args.push((q.minimum_bytes.min(i64::MAX as u64) as i64).into());
        }
        if q.suggestions {
            cond.push_str(" AND (rule_id IS NOT NULL OR logical>=104857600)");
            if q.risk.as_deref() != Some("protected") {
                cond.push_str(" AND risk NOT IN ('protected','keep') AND complete=1 AND blocked=0 AND path_key<>?");
                args.push(scan_root.into());
            }
        }
        let total = c.query_row(
            &format!("SELECT count(*) FROM entries WHERE {cond}"),
            rusqlite::params_from_iter(&args),
            |r| r.get(0),
        )?;
        let sort = match q.sort.as_deref() {
            Some("name") => "path_key",
            Some("activity") => "latest_change DESC",
            _ => "logical DESC",
        };
        args.push(q.limit.clamp(1, 200).into());
        args.push(q.offset.into());
        let mut stmt = c.prepare(&format!(
            "SELECT {FIELDS} FROM entries WHERE {cond} ORDER BY {sort},id LIMIT ? OFFSET ?"
        ))?;
        let items = stmt
            .query_map(rusqlite::params_from_iter(&args), decode)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(EntryPage { items, total })
    }
    pub fn pending(&self, scan: &str, limit: usize) -> Result<Vec<FileRecord>> {
        let c = self.connection()?;
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
    pub fn aggregate(&self, scan: &str) -> Result<()> {
        let mut c = self.connection()?;
        // One file identity is charged once. Logical size still describes all directory entries.
        c.execute("UPDATE entries SET allocated=0 WHERE scan_id=?1 AND is_dir=0 AND identity IS NOT NULL AND id NOT IN (SELECT MIN(id) FROM entries WHERE scan_id=?1 AND is_dir=0 AND identity IS NOT NULL GROUP BY identity)",[scan])?;
        let dirs: Vec<(i64, String)> = {
            let mut s=c.prepare("SELECT id,path_key FROM entries WHERE scan_id=?1 AND is_dir=1 ORDER BY length(path_key) DESC")?;
            let rows = s
                .query_map([scan], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            rows
        };
        let tx = c.transaction()?;
        for (id, path) in dirs {
            let (logical,alloc,files,complete,blocked,changed):(u64,Option<u64>,u64,bool,bool,i64)=tx.query_row("SELECT COALESCE(SUM(logical),0), CASE WHEN COUNT(*)=COUNT(allocated) THEN COALESCE(SUM(allocated),0) ELSE NULL END,COALESCE(SUM(file_count),0),COALESCE(MIN(complete),1),COALESCE(MAX(blocked OR risk='protected'),0),COALESCE(MAX(latest_change),0) FROM entries WHERE scan_id=?1 AND parent_key=?2",params![scan,path],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
            tx.execute("UPDATE entries SET logical=?2,allocated=?3,file_count=?4,complete=complete AND enumerated AND ?5,blocked=?6,latest_change=MAX(latest_change,?7) WHERE id=?1",params![id,logical,alloc,files,complete,blocked,changed])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn stats(&self, scan: &str) -> Result<(u64, u64, u64)> {
        Ok(self.connection()?.query_row("SELECT COALESCE(SUM(is_dir=0),0),COALESCE(SUM(is_dir=1),0),COALESCE(SUM(issue IS NOT NULL),0) FROM entries WHERE scan_id=?1",[scan],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)
    }
    pub fn groups(&self, scan: &str, kind: &str) -> Result<Vec<Group>> {
        let field = match kind {
            "risk" => "risk",
            "category" => "COALESCE(json_extract(assessment,'$.category'),'unknown')",
            _ => "COALESCE(owner,'未知')",
        };
        let c = self.connection()?;
        let mut s=c.prepare(&format!("SELECT {field},SUM(COALESCE(allocated,logical)),count(*) FROM entries WHERE scan_id=?1 AND is_dir=0 GROUP BY {field} ORDER BY SUM(COALESCE(allocated,logical)) DESC LIMIT 100"))?;
        let result = s
            .query_map([scan], |r| {
                Ok(Group {
                    name: r.get(0)?,
                    bytes: r.get(1)?,
                    count: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(result)
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
        self.connection()?.execute(
            "INSERT INTO history VALUES(?1,?2,?3)",
            params![h.id, h.time, serde_json::to_string(h)?],
        )?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<HistoryItem>> {
        let c = self.connection()?;
        let mut s = c.prepare("SELECT data FROM history ORDER BY time DESC LIMIT 500")?;
        let result = s
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|v| Ok(serde_json::from_str(&v?)?))
            .collect();
        result
    }
    pub fn save_analysis(&self, a: &AnalysisResult) -> Result<()> {
        self.connection()?.execute(
            "INSERT OR REPLACE INTO analyses VALUES(?1,?2,?3,?4,?5)",
            params![
                a.id,
                a.scan_id,
                a.entry_id,
                a.created,
                serde_json::to_string(a)?
            ],
        )?;
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
    pub fn recover_interrupted(&self) -> Result<()> {
        for mut s in self.scans()? {
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
        assert_eq!(store.groups("s", "category").unwrap().len(), 2);
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

use super::{decode, Store, FIELDS};
use anyhow::Result;
use cleaner_domain::FileRecord;
use cleaner_platform::normalize;
use rusqlite::params;

impl Store {
    pub fn preview_children(
        &self,
        scan: &str,
        parent: &str,
        limit: usize,
    ) -> Result<(Vec<FileRecord>, bool)> {
        let limit = limit.clamp(1, 60);
        let c = self.connection()?;
        let mut statement = c.prepare(&format!(
            "SELECT {FIELDS} FROM entries INDEXED BY entries_parent_size
             WHERE scan_id=?1 AND parent_key=?2 ORDER BY logical DESC,id LIMIT ?3"
        ))?;
        let mut rows = statement
            .query_map(params![scan, normalize(parent), (limit + 1) as i64], decode)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let truncated = rows.len() > limit;
        rows.truncate(limit);
        Ok((rows, truncated))
    }

    pub fn subtree_page(&self, scan: &str, path: &str, after: &str) -> Result<Vec<FileRecord>> {
        let key = normalize(path);
        let c = self.connection()?;
        let mut statement = c.prepare(&format!(
            "SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND path_key>?2
             AND (path_key=?3 OR (path_key>=?4 AND path_key<?5)) ORDER BY path_key LIMIT 512"
        ))?;
        let rows = statement
            .query_map(
                params![scan, after, key, format!("{key}\\"), format!("{key}]")],
                decode,
            )?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn refresh_subtree_protection(&self, scan: &str, path: &str) -> Result<()> {
        let key = normalize(path);
        let mut c = self.connection()?;
        let mut paths: Vec<String> = {
            let mut s = c.prepare(
                "SELECT path_key FROM entries WHERE scan_id=?1 AND is_dir=1
                AND (path_key=?2 OR (path_key>=?3 AND path_key<?4)) ORDER BY length(path_key) DESC",
            )?;
            let rows = s
                .query_map(
                    params![scan, key, format!("{key}\\"), format!("{key}]")],
                    |r| r.get(0),
                )?
                .collect::<rusqlite::Result<_>>()?;
            rows
        };
        paths.extend(
            key.match_indices('\\')
                .rev()
                .map(|(i, _)| key[..i].to_string()),
        );
        let tx = c.transaction()?;
        {
            let mut s = tx.prepare_cached("UPDATE entries SET blocked=COALESCE((SELECT MAX(child.blocked OR child.risk='protected')
                FROM entries child WHERE child.scan_id=?1 AND child.parent_key=?2),0)
                WHERE scan_id=?1 AND path_key=?2 AND is_dir=1")?;
            for path in paths {
                s.execute(params![scan, path])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

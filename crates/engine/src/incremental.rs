//! Journal-proven snapshots only. Changed identities invalidate whole affected subtrees.
use crate::store::Store;
use anyhow::{bail, Result};
use cleaner_domain::FileRecord;
use cleaner_platform::{filesystem, normalize, within};
use rusqlite::params;
use std::path::Path;

pub fn seed(
    store: &Store,
    previous: &str,
    next: &str,
    root: &FileRecord,
    changed: &[u128],
) -> Result<usize> {
    let serial = root
        .identity
        .as_deref()
        .and_then(|s| s.split_once(':'))
        .map(|p| p.0)
        .ok_or_else(|| anyhow::anyhow!("卷身份不可用"))?;
    let mut connection = store.connection()?;
    connection.execute_batch("CREATE TEMP TABLE changed_ids(identity TEXT PRIMARY KEY);")?;
    {
        let tx = connection.transaction()?;
        for id in changed {
            tx.execute(
                "INSERT OR IGNORE INTO changed_ids VALUES(?1)",
                [format!("{serial}:{id:032x}")],
            )?;
        }
        tx.commit()?;
    }
    // Both parents and touched file IDs are included. This also refreshes other in-scope hardlinks.
    let mut paths: Vec<String> = {
        let mut s = connection.prepare("SELECT DISTINCT CASE WHEN is_dir=1 THEN path_key ELSE parent_key END FROM entries WHERE scan_id=?1 AND identity IN (SELECT identity FROM changed_ids)")?;
        let rows = s
            .query_map([previous], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        rows
    };
    paths.sort_by_key(String::len);
    let mut targets: Vec<FileRecord> = Vec::new();
    for path in paths {
        if targets.iter().any(|p| within(&path, &p.path)) {
            continue;
        }
        let old = store.by_path(previous, &path)?;
        let fresh = filesystem::inspect(Path::new(&old.path))?;
        if !fresh.is_dir
            || fresh.identity != old.identity
            || fresh.attributes & (filesystem::REPARSE | filesystem::RECALL | filesystem::OFFLINE)
                != 0
        {
            bail!("变化目录身份不一致，回退完整扫描");
        }
        targets.push(fresh);
    }
    let tx = connection.transaction()?;
    let columns = "path_key,parent_key,is_dir,identity,logical,allocated,file_count,complete,blocked,latest_change,enumerated,issue,risk,owner,rule_id,data,assessment";
    tx.execute(&format!("INSERT INTO entries(scan_id,{columns}) SELECT ?1,{columns} FROM entries WHERE scan_id=?2"), params![next,previous])?;
    // Restore unaggregated values. In particular a hardlink that was previously charged zero
    // must be counted again if the originally charged link has disappeared.
    tx.execute("UPDATE entries SET allocated=json_extract(data,'$.allocatedBytes'),latest_change=json_extract(data,'$.modified'),blocked=0,complete=CASE WHEN issue IS NULL AND enumerated=1 THEN 1 ELSE 0 END WHERE scan_id=?1", [next])?;
    for target in &targets {
        tx.execute("WITH RECURSIVE subtree(id,path_key) AS (SELECT id,path_key FROM entries WHERE scan_id=?1 AND path_key=?2 UNION ALL SELECT e.id,e.path_key FROM entries e JOIN subtree t ON e.parent_key=t.path_key WHERE e.scan_id=?1) DELETE FROM entries WHERE id IN (SELECT id FROM subtree)",params![next,normalize(&target.path)])?;
    }
    tx.commit()?;
    let count = targets.len();
    Store::insert_batch(&mut connection, next, &targets)?;
    Ok(count)
}

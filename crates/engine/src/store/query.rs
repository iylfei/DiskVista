use super::{decode, Store, FIELDS};
use anyhow::Result;
use cleaner_domain::{EntryPage, EntryQuery};
use cleaner_platform::normalize;
use rusqlite::{types::Value, OptionalExtension};

fn table(query: &EntryQuery) -> &'static str {
    // A small directory must not walk the whole scan's size index to fill LIMIT.
    if query.parent.is_some() {
        "entries INDEXED BY entries_parent"
    } else {
        "entries"
    }
}

fn count_sql(query: &EntryQuery, conditions: &str) -> String {
    format!("SELECT count(*) FROM {} WHERE {conditions}", table(query))
}

fn page_sql(query: &EntryQuery, conditions: &str) -> String {
    let table = if query.parent.is_some()
        && !matches!(
            query.sort.as_deref(),
            Some("name" | "activity" | "activity_desc" | "activity_asc")
        ) {
        "entries INDEXED BY entries_parent_size"
    } else {
        table(query)
    };
    let sort = match query.sort.as_deref() {
        Some("name") => "path_key",
        Some("activity" | "activity_desc") => "latest_change DESC",
        Some("activity_asc") => "latest_change ASC",
        _ => "logical DESC",
    };
    format!("SELECT {FIELDS} FROM {table} WHERE {conditions} ORDER BY {sort},id LIMIT ? OFFSET ?")
}

impl Store {
    pub fn query(&self, q: &EntryQuery) -> Result<EntryPage> {
        anyhow::ensure!(q.analysis_status.is_empty(), "AI 分析筛选需要当前有效结果");
        let c = self.connection()?;
        let scan_root: Option<String> = c
            .query_row("SELECT root FROM scans WHERE id=?1", [&q.scan_id], |r| {
                r.get(0)
            })
            .optional()?;
        let scan_root = scan_root.map(|root| normalize(&root)).unwrap_or_default();
        let mut cond = String::from("scan_id=?");
        let mut args: Vec<Value> = vec![q.scan_id.clone().into()];
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
            &count_sql(q, &cond),
            rusqlite::params_from_iter(&args),
            |r| r.get(0),
        )?;
        args.push(q.limit.clamp(1, 200).into());
        args.push(q.offset.into());
        let mut stmt = c.prepare(&page_sql(q, &cond))?;
        let items = stmt
            .query_map(rusqlite::params_from_iter(&args), decode)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(EntryPage { items, total })
    }
}

#[cfg(test)]
mod tests;

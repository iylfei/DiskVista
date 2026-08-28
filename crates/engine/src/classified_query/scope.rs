use crate::{
    analysis_filter::AnalysisFilter,
    store::{decode, Store, FIELDS},
};
use anyhow::{anyhow, Result};
use cleaner_domain::{EntryQuery, FileRecord};
use cleaner_platform::normalize;
use rusqlite::types::Value;
use std::collections::HashMap;

pub(super) fn normalized(query: &EntryQuery) -> EntryQuery {
    let mut query = query.clone();
    query.offset = 0;
    query.limit = 0;
    query.parent = query.parent.as_deref().map(normalize);
    query.search = query
        .search
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(normalize);
    query.risk = query.risk.filter(|value| !value.is_empty());
    query.sort = Some(
        match query.sort.as_deref() {
            Some("name") => "name",
            Some("activity" | "activity_desc") => "activity_desc",
            Some("activity_asc") => "activity_asc",
            _ => "size",
        }
        .into(),
    );
    query
}

pub(super) fn visit(
    store: &Store,
    query: &EntryQuery,
    only_files: bool,
    ordered: bool,
    analysis: Option<&AnalysisFilter>,
    mut read: impl FnMut(FileRecord) -> Result<bool>,
) -> Result<()> {
    let mut conditions = String::from("scan_id=?");
    let mut args: Vec<Value> = vec![query.scan_id.clone().into()];
    if query.directories_only {
        conditions.push_str(" AND is_dir=1");
    }
    if only_files {
        conditions.push_str(" AND is_dir=0");
    }
    if query.issues_only {
        conditions.push_str(" AND issue IS NOT NULL");
    }
    if let Some(parent) = &query.parent {
        conditions.push_str(" AND parent_key=?");
        args.push(normalize(parent).into());
    }
    if let Some(search) = query.search.as_ref().filter(|value| !value.is_empty()) {
        conditions.push_str(" AND instr(path_key,?)>0");
        args.push(normalize(search).into());
    }
    if query.minimum_bytes > 0 {
        conditions.push_str(" AND logical>=?");
        args.push((query.minimum_bytes.min(i64::MAX as u64) as i64).into());
    }
    if let Some(analysis) = analysis {
        analysis.append_sql(&mut conditions, &mut args)?;
    }
    let order = if ordered {
        match query.sort.as_deref() {
            Some("name") => "path_key,id",
            Some("activity" | "activity_desc") => "latest_change DESC,id",
            Some("activity_asc") => "latest_change ASC,id",
            _ => "logical DESC,id",
        }
    } else {
        "id"
    };
    let connection = store.connection()?;
    let mut statement = connection.prepare(&format!(
        "SELECT {FIELDS} FROM entries WHERE {conditions} ORDER BY {order}"
    ))?;
    let mut rows = statement.query(rusqlite::params_from_iter(&args))?;
    while let Some(row) = rows.next()? {
        if !read(decode(row)?)? {
            break;
        }
    }
    Ok(())
}

pub(super) fn load(store: &Store, scan: &str, ids: &[i64]) -> Result<Vec<FileRecord>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut args: Vec<Value> = vec![scan.to_owned().into()];
    args.extend(ids.iter().map(|id| (*id).into()));
    let placeholders = vec!["?"; ids.len()].join(",");
    let connection = store.connection()?;
    let mut statement = connection.prepare(&format!(
        "SELECT {FIELDS} FROM entries WHERE scan_id=? AND id IN ({placeholders})"
    ))?;
    let mut files = statement
        .query_map(rusqlite::params_from_iter(&args), decode)?
        .map(|row| row.map(|file| (file.id, file)))
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;
    ids.iter()
        .map(|id| {
            files
                .remove(id)
                .ok_or_else(|| anyhow!("扫描记录已变化，请刷新后重试"))
        })
        .collect()
}

use anyhow::{bail, Result};
use cleaner_domain::{AnalysisContext, ContextFile, Evidence, HistoryReference};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub const MAX_BATCH_ITEMS: usize = 20;
pub const MAX_BATCH_METADATA_BYTES: usize = 65_536;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchMetadata<'a> {
    scan_id: &'a str,
    directories: Vec<&'a str>,
    notes: Vec<&'a str>,
    history_references: Vec<SharedHistory<'a>>,
    items: Vec<MetadataItem<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetadataItem<'a> {
    entry_id: i64,
    directory_id: usize,
    name: &'a str,
    logical_bytes: u64,
    file_count: u64,
    modified: i64,
    accessed: i64,
    evidence: &'a [Evidence],
    files: &'a [ContextFile],
    history_references: Vec<ItemHistory<'a>>,
    truncated: bool,
    note_id: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemHistory<'a> {
    id: &'a str,
    match_basis: &'a [String],
}

#[derive(PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SharedHistory<'a> {
    id: &'a str,
    path: &'a str,
    bytes: u64,
    recycled_at: i64,
    owner: Option<&'a str>,
    category: Option<&'a str>,
}

impl<'a> From<&'a HistoryReference> for SharedHistory<'a> {
    fn from(value: &'a HistoryReference) -> Self {
        Self {
            id: &value.id,
            path: &value.path,
            bytes: value.bytes,
            recycled_at: value.recycled_at,
            owner: value.owner.as_deref(),
            category: value.category.as_deref(),
        }
    }
}

fn metadata(contexts: &[AnalysisContext]) -> Result<BatchMetadata<'_>> {
    if contexts.is_empty() || contexts.len() > MAX_BATCH_ITEMS {
        bail!("每个 AI 请求须包含 1 到 {MAX_BATCH_ITEMS} 个项目");
    }
    let scan_id = &contexts[0].scan_id;
    let mut entries = HashSet::new();
    let mut directories = Vec::new();
    let mut notes = Vec::new();
    let mut history_references: Vec<SharedHistory<'_>> = Vec::new();
    let mut history_indexes = HashMap::new();
    let mut items = Vec::with_capacity(contexts.len());
    for context in contexts {
        if &context.scan_id != scan_id || !entries.insert(context.entry_id) {
            bail!("批量分析必须来自同一扫描，且文件编号不能重复");
        }
        let (directory, name) = context
            .path
            .rfind(['/', '\\'])
            .map(|index| context.path.split_at(index + 1))
            .unwrap_or(("", context.path.as_str()));
        let directory_id = intern(&mut directories, directory);
        let note_id = intern(&mut notes, &context.note);
        let mut allowed_history = HashSet::new();
        if context.history_references.len() > 100 {
            bail!("单个文件的回收历史参考超过限制");
        }
        for reference in &context.history_references {
            if !reference.id.starts_with("history:")
                || reference.id.len() <= "history:".len()
                || reference.id.len() > 256
                || reference.id.chars().any(char::is_control)
                || !allowed_history.insert(&reference.id)
            {
                bail!("回收历史参考编号无效或重复");
            }
            let shared = SharedHistory::from(reference);
            if let Some(index) = history_indexes.get(reference.id.as_str()) {
                if history_references[*index] != shared {
                    bail!("同一回收历史的快照不一致，请重新准备分析");
                }
            } else {
                history_indexes.insert(reference.id.as_str(), history_references.len());
                history_references.push(shared);
            }
        }
        items.push(MetadataItem {
            entry_id: context.entry_id,
            directory_id,
            name,
            logical_bytes: context.logical_bytes,
            file_count: context.file_count,
            modified: context.modified,
            accessed: context.accessed,
            evidence: &context.evidence,
            files: &context.files,
            history_references: context
                .history_references
                .iter()
                .map(|reference| ItemHistory {
                    id: &reference.id,
                    match_basis: &reference.match_basis,
                })
                .collect(),
            truncated: context.truncated,
            note_id,
        });
    }
    Ok(BatchMetadata {
        scan_id,
        directories,
        notes,
        history_references,
        items,
    })
}

fn intern<'a>(values: &mut Vec<&'a str>, value: &'a str) -> usize {
    match values.iter().position(|existing| *existing == value) {
        Some(index) => index,
        None => {
            values.push(value);
            values.len() - 1
        }
    }
}

/// Serialized metadata size after sharing directories, notes and history snapshots.
pub fn batch_metadata_bytes(contexts: &[AnalysisContext]) -> Result<usize> {
    Ok(serde_json::to_vec(&metadata(contexts)?)?.len())
}

pub fn validate_batch_contexts(contexts: &[AnalysisContext]) -> Result<()> {
    check_size(batch_metadata_bytes(contexts)?)
}

fn check_size(bytes: usize) -> Result<()> {
    if bytes > MAX_BATCH_METADATA_BYTES {
        bail!("批量分析的元数据超过 64 KiB，请减少项目数量");
    }
    Ok(())
}

pub(crate) fn payload(contexts: &[AnalysisContext], samples: Option<&Value>) -> Result<String> {
    if contexts.len() != 1 && samples.is_some() {
        bail!("正文样本只允许用于单项分析");
    }
    let metadata = metadata(contexts)?;
    check_size(serde_json::to_vec(&metadata)?.len())?;
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload<'a> {
        metadata: BatchMetadata<'a>,
        #[serde(skip_serializing_if = "Option::is_none")]
        authorized_text_samples: Option<&'a Value>,
    }
    Ok(serde_json::to_string(&Payload {
        metadata,
        authorized_text_samples: samples,
    })?)
}

#[cfg(test)]
mod tests;

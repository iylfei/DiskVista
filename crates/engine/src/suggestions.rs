use crate::{
    rules::RuleSet,
    safety::SafetyPolicy,
    store::{decode, Store, FIELDS},
};
use anyhow::{bail, Result};
use cleaner_domain::*;
use cleaner_platform::{normalize, within};
use std::collections::{BTreeMap, HashMap};

pub struct SuggestionIndex {
    entries: Vec<FileRecord>,
    names: HashMap<String, String>,
}

pub fn build(store: &Store, scan_id: &str) -> Result<SuggestionIndex> {
    let scan = store.require_finished(scan_id)?;
    let root = normalize(&scan.root);
    let settings = store.settings()?;
    let rules = RuleSet::load(settings.community_enabled)?;
    let policy = SafetyPolicy::new(settings);
    let apps = store.apps(scan_id)?;
    let c = store.connection()?;
    let mut statement = c.prepare(&format!("SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND (rule_id IS NOT NULL OR (is_dir=0 AND logical>=104857600))"))?;
    let mut entries = Vec::new();
    for file in statement.query_map([scan_id], decode)? {
        let mut file = file?;
        file.assessment = rules.classify(&file, &policy, &apps);
        let protected_descendant = file.is_dir
            && policy
                .settings
                .protected_paths
                .iter()
                .chain(&policy.settings.ignored_paths)
                .any(|path| within(path, &file.path));
        if normalize(&file.path) == root
            || !file.complete
            || file.has_blocked_children
            || protected_descendant
        {
            file.assessment.risk = "protected".into();
            file.assessment.protected_reason =
                Some("扫描起点、未能扫描完整或包含受保护内容，不能整体清理".into());
        }
        if file.assessment.rule_id.is_some() || (!file.is_dir && file.logical_bytes >= 104_857_600)
        {
            entries.push(file);
        }
    }
    Ok(SuggestionIndex {
        entries,
        names: rules
            .rules
            .into_iter()
            .map(|rule| (rule.id, rule.name))
            .collect(),
    })
}

fn group(file: &FileRecord) -> String {
    file.assessment
        .rule_id
        .as_ref()
        .map(|id| format!("rule:{id}"))
        .unwrap_or_else(|| "large-files".into())
}

fn matches(file: &FileRecord, query: &SuggestionQuery) -> bool {
    let a = &file.assessment;
    let risk = match query.risk.as_str() {
        "protected" => a.risk == "protected",
        "low" => a.risk == "low",
        "unknown" => {
            !matches!(a.risk.as_str(), "protected" | "keep")
                && a.rule_id.is_none()
                && a.owner.is_none()
        }
        "known" => {
            !matches!(a.risk.as_str(), "protected" | "keep")
                && (a.rule_id.is_some() || a.owner.is_some())
        }
        _ => !matches!(a.risk.as_str(), "protected" | "keep"),
    };
    risk && (query.search.is_empty() || normalize(&file.path).contains(&normalize(&query.search)))
}

fn candidates<'a>(index: &'a SuggestionIndex, query: &SuggestionQuery) -> Vec<&'a FileRecord> {
    let filtered: Vec<_> = index.entries.iter().filter(|f| matches(f, query)).collect();
    let directories: HashMap<_, _> = filtered
        .iter()
        .filter(|f| f.is_dir)
        .map(|f| (normalize(&f.path), group(f)))
        .collect();
    filtered
        .into_iter()
        .filter(|file| {
            let key = normalize(&file.path);
            let own_group = group(file);
            !key.match_indices('\\')
                .any(|(end, _)| directories.get(&key[..end]) == Some(&own_group))
        })
        .collect()
}

pub fn page(index: &SuggestionIndex, query: &SuggestionQuery) -> SuggestionPage {
    let files = candidates(index, query);
    let mut groups: BTreeMap<String, SuggestionGroup> = BTreeMap::new();
    for file in &files {
        let id = group(file);
        let recognized = file.assessment.rule_id.is_some();
        let item = groups.entry(id.clone()).or_insert_with(|| SuggestionGroup {
            id,
            recognized,
            count: 0,
            occupied_bytes: 0,
            estimated: false,
            name: file
                .assessment
                .rule_id
                .as_ref()
                .and_then(|id| index.names.get(id))
                .cloned()
                .unwrap_or_else(|| "大文件".into()),
            purpose: if recognized {
                file.assessment.purpose.clone()
            } else {
                "大于 100 MiB 的文件。请先确认里面的内容是否还需要。".into()
            },
            consequence: if recognized {
                file.assessment.consequence.clone()
            } else {
                "可能包含视频、安装包或个人资料；文件大不代表可以删除。".into()
            },
        });
        item.count += 1;
        item.occupied_bytes = item
            .occupied_bytes
            .saturating_add(file.allocated_bytes.unwrap_or(file.logical_bytes));
        item.estimated |= file.allocated_bytes.is_none();
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by(|a, b| {
        b.recognized
            .cmp(&a.recognized)
            .then(b.occupied_bytes.cmp(&a.occupied_bytes))
            .then(a.name.cmp(&b.name))
    });
    let mut files: Vec<_> = files
        .into_iter()
        .filter(|f| query.group.as_ref().is_none_or(|id| group(f) == *id))
        .collect();
    files.sort_by(|a, b| {
        let order = match query.sort.as_str() {
            "name" => a.path.cmp(&b.path),
            "activity" => b.latest_change.cmp(&a.latest_change),
            _ => b
                .allocated_bytes
                .unwrap_or(b.logical_bytes)
                .cmp(&a.allocated_bytes.unwrap_or(a.logical_bytes)),
        };
        order.then(a.id.cmp(&b.id))
    });
    SuggestionPage {
        groups,
        total: files.len(),
        items: files
            .into_iter()
            .skip(query.offset)
            .take(query.limit.clamp(1, 100))
            .cloned()
            .collect(),
    }
}

pub fn selection(index: &SuggestionIndex, query: &SuggestionQuery) -> Result<Vec<FileRecord>> {
    let Some(id) = query.group.as_ref().filter(|id| id.starts_with("rule:")) else {
        bail!("请逐项选择用途未识别的大文件");
    };
    let files: Vec<_> = candidates(index, query)
        .into_iter()
        .filter(|f| group(f) == *id && f.assessment.risk != "protected")
        .cloned()
        .collect();
    if files.len() > 500 {
        bail!("这一类超过 500 项，请先缩小搜索范围或按页选择");
    }
    Ok(files)
}

#[cfg(test)]
mod tests;

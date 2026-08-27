mod query;

use crate::{
    application_index::ApplicationIndex,
    rules::RuleSet,
    safety::SafetyPolicy,
    store::{decode, Store, FIELDS},
};
use anyhow::Result;
use cleaner_domain::*;
use cleaner_platform::{normalize, within};
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
};

pub use query::{page, selection};

struct Candidate {
    id: i64,
    path: String,
    key: String,
    is_dir: bool,
    occupied: u64,
    estimated: bool,
    latest_change: i64,
    assessment: usize,
    group: usize,
}

pub struct SuggestionIndex {
    store: Store,
    scan_id: String,
    entries: Vec<Candidate>,
    assessments: Vec<Assessment>,
    groups: Vec<SuggestionGroup>,
    views: Mutex<VecDeque<query::View>>,
}

impl SuggestionIndex {
    fn from_records(
        store: &Store,
        scan: &str,
        names: HashMap<String, String>,
        records: impl Iterator<Item = Result<FileRecord>>,
    ) -> Result<Self> {
        let mut index = Self {
            store: store.clone(),
            scan_id: scan.into(),
            entries: vec![],
            assessments: vec![],
            groups: vec![],
            views: Mutex::new(VecDeque::new()),
        };
        let mut assessments = HashMap::new();
        let mut groups = HashMap::new();
        for record in records {
            let file = record?;
            let a = file.assessment;
            let assessment_key = serde_json::to_string(&a)?;
            let assessment = *assessments.entry(assessment_key).or_insert_with(|| {
                index.assessments.push(a.clone());
                index.assessments.len() - 1
            });
            let group_id = a
                .rule_id
                .as_ref()
                .map(|id| format!("rule:{id}"))
                .unwrap_or_else(|| "large-files".into());
            let group = *groups.entry(group_id.clone()).or_insert_with(|| {
                let recognized = a.rule_id.is_some();
                index.groups.push(SuggestionGroup {
                    id: group_id,
                    recognized,
                    count: 0,
                    occupied_bytes: 0,
                    estimated: false,
                    name: a
                        .rule_id
                        .as_ref()
                        .and_then(|id| names.get(id))
                        .cloned()
                        .unwrap_or_else(|| "大文件".into()),
                    purpose: if recognized {
                        a.purpose.clone()
                    } else {
                        "大于 100 MiB 的文件。请先确认里面的内容是否还需要。".into()
                    },
                    consequence: if recognized {
                        a.consequence.clone()
                    } else {
                        "可能包含视频、安装包或个人资料；文件大不代表可以删除。".into()
                    },
                });
                index.groups.len() - 1
            });
            index.entries.push(Candidate {
                id: file.id,
                key: normalize(&file.path),
                path: file.path,
                is_dir: file.is_dir,
                occupied: file.allocated_bytes.unwrap_or(file.logical_bytes),
                estimated: file.allocated_bytes.is_none(),
                latest_change: file.latest_change,
                assessment,
                group,
            });
        }
        Ok(index)
    }

    fn load(&self, selected: &[usize]) -> Result<Vec<FileRecord>> {
        let c = self.store.connection()?;
        let mut statement = c.prepare_cached(&format!(
            "SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND id=?2"
        ))?;
        selected
            .iter()
            .map(|&i| {
                let candidate = &self.entries[i];
                let mut file =
                    statement.query_row(rusqlite::params![self.scan_id, candidate.id], decode)?;
                file.assessment = self.assessments[candidate.assessment].clone();
                Ok(file)
            })
            .collect()
    }
}

pub fn build(store: &Store, scan_id: &str) -> Result<SuggestionIndex> {
    let scan = store.require_finished(scan_id)?;
    let root = normalize(&scan.root);
    let settings = store.settings()?;
    let rules = RuleSet::load(settings.community_enabled)?;
    let policy = SafetyPolicy::new(settings);
    let apps = ApplicationIndex::new(&store.apps(scan_id)?, &policy);
    let c = store.connection()?;
    // Disjoint ranges allow partial indexes even when a snapshot has no candidates.
    let mut statement = c.prepare(&format!(
        "SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND rule_id IS NOT NULL
         UNION ALL SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND rule_id IS NULL AND is_dir=0 AND logical>=104857600"
    ))?;
    let files = statement.query_map([scan_id], decode)?;
    let names = rules
        .rules
        .iter()
        .map(|r| (r.id.clone(), r.name.clone()))
        .collect();
    let records = files
        .map(|file| -> Result<FileRecord> {
            let mut file = file?;
            file.assessment = rules.classify_indexed(&file, &policy, &apps);
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
            Ok(file)
        })
        .filter(|file| match file {
            Ok(f) => {
                f.assessment.rule_id.is_some() || (!f.is_dir && f.logical_bytes >= 104_857_600)
            }
            Err(_) => true,
        });
    SuggestionIndex::from_records(store, scan_id, names, records)
}

#[cfg(test)]
mod tests;

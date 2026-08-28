mod query;
mod seeds;

use crate::{
    analysis_filter::AnalysisFilter,
    application_index::ApplicationIndex,
    classified_query::Classifier,
    rules::RuleSet,
    safety::SafetyPolicy,
    store::{decode, Store, FIELDS},
};
use anyhow::Result;
use cleaner_domain::*;
use cleaner_platform::normalize;
use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
};

pub use query::{page, page_with_analysis, selection, selection_with_analysis};

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
    started: i64,
    valid_until: Option<i64>,
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
            started: chrono::Utc::now().timestamp(),
            valid_until: None,
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

    pub fn is_current(&self) -> bool {
        self.valid_at(chrono::Utc::now().timestamp())
    }

    fn valid_at(&self, now: i64) -> bool {
        now >= self.started && self.valid_until.is_none_or(|until| now < until)
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
    let policy = SafetyPolicy::new(store.settings()?);
    let apps = ApplicationIndex::for_scan(store, scan_id, &policy)?;
    build_indexed(store, scan_id, &policy, &apps)
}

pub fn build_indexed(
    store: &Store,
    scan_id: &str,
    policy: &SafetyPolicy,
    apps: &ApplicationIndex,
) -> Result<SuggestionIndex> {
    let rules = RuleSet::load(policy.settings.community_enabled)?;
    build_with_rules(store, scan_id, policy, apps, &rules)
}

fn build_with_rules(
    store: &Store,
    scan_id: &str,
    policy: &SafetyPolicy,
    apps: &ApplicationIndex,
    rules: &RuleSet,
) -> Result<SuggestionIndex> {
    let scan = store.require_finished(scan_id)?;
    let started = chrono::Utc::now().timestamp();
    let classifier = Classifier::new(&scan, rules, policy, apps);
    let mut valid_until = None;
    let root = normalize(&scan.root);
    let c = store.connection()?;
    let (sql, parameters) = seeds::query(scan_id, &root, rules);
    let mut statement = c.prepare(&sql)?;
    let files = statement.query_map(rusqlite::params_from_iter(parameters), decode)?;
    let names = rules
        .rules
        .iter()
        .map(|r| (r.id.clone(), r.name.clone()))
        .collect();
    let records = files
        .map(|file| -> Result<FileRecord> {
            let mut file = file?;
            if let Some(next) = classifier.next_change(&file, started) {
                valid_until = Some(valid_until.map_or(next, |old: i64| old.min(next)));
            }
            classifier.apply(&mut file);
            Ok(file)
        })
        .filter(|file| match file {
            Ok(f) => {
                f.assessment.rule_id.is_some() || (!f.is_dir && f.logical_bytes >= 104_857_600)
            }
            Err(_) => true,
        });
    let mut index = SuggestionIndex::from_records(store, scan_id, names, records)?;
    index.started = started;
    index.valid_until = valid_until;
    Ok(index)
}

#[cfg(test)]
mod tests;

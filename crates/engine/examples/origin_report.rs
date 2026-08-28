//! Compare attribution on recorded entries only; never scans or changes the database.
use anyhow::{ensure, Result};
use cleaner_engine::{
    application_index::ApplicationIndex, rules::RuleSet, safety::SafetyPolicy, store::Store,
};
use cleaner_platform::normalize;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    time::Instant,
};

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Counts {
    entries: u64,
    files: u64,
    directories: u64,
    old_known: u64,
    old_unknown: u64,
    new_known: u64,
    new_unknown: u64,
    newly_attributed: u64,
    lost_attribution: u64,
    changed_owner: u64,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct AddedOwner {
    count: u64,
    confidence: BTreeMap<String, u64>,
    roots: BTreeSet<String>,
    sources: BTreeSet<String>,
}

#[derive(Default, Serialize)]
struct RemovedOwner {
    count: u64,
    samples: BTreeSet<String>,
}

fn uri_path(path: &str) -> String {
    path.replace('%', "%25")
        .replace('?', "%3f")
        .replace('#', "%23")
        .replace('\\', "/")
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("existing database path required"))?;
    let scan_id = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!("scan id required"))?;
    ensure!(
        std::path::Path::new(&path).is_file(),
        "existing database required"
    );
    let before = std::fs::metadata(&path)?;
    let store = Store {
        path: PathBuf::from(format!("file:{}?mode=ro", uri_path(&path))),
    };
    let guard = store.connection()?;
    ensure!(guard.is_readonly("main")?, "read-only database required");
    let scan = store.scan(&scan_id)?;
    let policy = SafetyPolicy::new(store.settings()?);
    let rules = RuleSet::load(policy.settings.community_enabled)?;
    let started = Instant::now();
    let index = ApplicationIndex::with_snapshot(&store, &scan_id, &store.apps(&scan_id)?, &policy)?;
    let index_ms = started.elapsed().as_millis();
    let mut counts = Counts::default();
    let mut owners = BTreeMap::<String, AddedOwner>::new();
    let mut removed = BTreeMap::<String, RemovedOwner>::new();
    let mut changes = BTreeMap::<String, u64>::new();
    let mut changed_samples = Vec::new();
    let mut sampled_changes = BTreeSet::new();
    let mut known_files = [0_u64; 2];
    let mut new_confidence = BTreeMap::<String, u64>::new();
    let mut after = 0;
    loop {
        let page = store.page_after(&scan_id, after, false)?;
        if page.is_empty() {
            break;
        }
        for file in page {
            after = file.id;
            counts.entries += 1;
            if file.is_dir {
                counts.directories += 1;
            } else {
                counts.files += 1;
            }
            let assessment = rules.classify_indexed(&file, &policy, &index);
            let old = file.assessment.owner.as_deref();
            let new = assessment.owner.as_deref();
            if !file.is_dir {
                known_files[0] += u64::from(old.is_some());
                known_files[1] += u64::from(new.is_some());
            }
            if old.is_some() {
                counts.old_known += 1;
            } else {
                counts.old_unknown += 1;
            }
            if new.is_some() {
                counts.new_known += 1;
            } else {
                counts.new_unknown += 1;
            }
            match (old, new) {
                (None, Some(name)) => {
                    counts.newly_attributed += 1;
                    *new_confidence
                        .entry(assessment.confidence.clone())
                        .or_default() += 1;
                    let entry = owners.entry(name.into()).or_default();
                    entry.count += 1;
                    *entry
                        .confidence
                        .entry(assessment.confidence.clone())
                        .or_default() += 1;
                    if let Some(origin) = index.origin(&normalize(&file.path)) {
                        if entry.roots.len() < 3 {
                            entry.roots.insert(origin.path.clone());
                        }
                        entry.sources.insert(origin.evidence.source.clone());
                    }
                }
                (Some(old), None) => {
                    counts.lost_attribution += 1;
                    let entry = removed.entry(old.into()).or_default();
                    entry.count += 1;
                    if entry.samples.len() < 3 {
                        entry.samples.insert(file.path.clone());
                    }
                }
                (Some(old), Some(new)) if old != new => {
                    counts.changed_owner += 1;
                    let key = format!("{old} -> {new}");
                    *changes.entry(key.clone()).or_default() += 1;
                    if changed_samples.len() < 20 && sampled_changes.insert(key) {
                        changed_samples.push(serde_json::json!({
                            "path":file.path,"oldOwner":old,"newOwner":new,
                            "newConfidence":assessment.confidence,
                            "oldEvidence":file.assessment.evidence,
                            "newEvidence":assessment.evidence,
                        }));
                    }
                }
                _ => {}
            }
        }
    }
    let mut owners: Vec<_> = owners.into_iter().collect();
    owners.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(&b.0)));
    let mut changes: Vec<_> = changes.into_iter().collect();
    changes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut removed: Vec<_> = removed.into_iter().collect();
    removed.sort_by(|a, b| b.1.count.cmp(&a.1.count).then(a.0.cmp(&b.0)));
    let after_metadata = std::fs::metadata(&path)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "scanId":scan.id,"scanStatus":scan.status,"scanRoot":scan.root,
            "partialSnapshot":scan.status != "complete", "readOnly":true,
            "databaseChanges":guard.total_changes(),
            "databaseSizeUnchanged":before.len()==after_metadata.len(),
            "databaseModifiedUnchanged":before.modified()?==after_metadata.modified()?,
            "indexMs":index_ms,"elapsedMs":started.elapsed().as_millis(),
            "counts":counts,"topNewOwners":owners.into_iter().take(10).collect::<Vec<_>>(),
            "fileOwnership":{
                "oldKnown":known_files[0], "oldUnknown":counts.files-known_files[0],
                "newKnown":known_files[1], "newUnknown":counts.files-known_files[1],
            },
            "newAttributionConfidence":new_confidence,
            "topOwnerChanges":changes.into_iter().take(10).collect::<Vec<_>>(),
            "topLostOwners":removed.into_iter().take(10).collect::<Vec<_>>(),
            "changedOwnerSamples":changed_samples,
        }))?
    );
    Ok(())
}

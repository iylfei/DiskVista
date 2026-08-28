use super::*;
use crate::{
    analysis_filter::AnalysisFilter,
    application_index::ApplicationIndex,
    rules::{Rule, RuleSet},
    safety::SafetyPolicy,
    store::Store,
};
use cleaner_domain::*;
use std::collections::BTreeSet;

const ROOT: &str = "D:\\QueryFixture";

fn file(path: &str, bytes: u64) -> FileRecord {
    FileRecord {
        path: path.into(),
        parent: path
            .rsplit_once('\\')
            .map(|(parent, _)| parent)
            .unwrap_or("")
            .into(),
        name: path.rsplit('\\').next().unwrap().into(),
        logical_bytes: bytes,
        allocated_bytes: Some(bytes + 1),
        modified: 1_700_000_000,
        accessed: 1_700_000_002,
        created: 1_600_000_000,
        latest_change: 1_700_000_001,
        complete: true,
        enumerated: true,
        file_count: 1,
        assessment: Assessment {
            risk: "review".into(),
            category: "unknown".into(),
            confidence: "low".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn rule(root: &str) -> Rule {
    Rule {
        id: "fixture-cache".into(),
        name: "Fixture cache".into(),
        root: root.into(),
        category: "cache".into(),
        owner: "Fixture app".into(),
        age_days: 14,
        purpose: "Fixture purpose".into(),
        consequence: "Fixture consequence".into(),
        recovery: "Fixture recovery".into(),
        warning: String::new(),
        community: false,
        excludes: Vec::new(),
        detect_files: Vec::new(),
    }
}

struct Fixture {
    _temp: tempfile::TempDir,
    store: Store,
    scan: Scan,
}

impl Fixture {
    fn new(files: &[FileRecord]) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("query.sqlite")).unwrap();
        let scan = Scan {
            id: "s".into(),
            root: ROOT.into(),
            started: 1_710_000_000,
            finished: Some(1_710_000_060),
            status: "complete".into(),
            files: files.len() as u64,
            logical_bytes: files.iter().map(|file| file.logical_bytes).sum(),
            ..Default::default()
        };
        store.save_scan(&scan).unwrap();
        Store::insert_batch(&mut store.connection().unwrap(), "s", files).unwrap();
        Self {
            _temp: temp,
            store,
            scan,
        }
    }

    fn raw_snapshot(&self) -> (String, Vec<Vec<rusqlite::types::Value>>) {
        let connection = self.store.connection().unwrap();
        let mut statement = connection
            .prepare("SELECT * FROM entries ORDER BY id")
            .unwrap();
        let columns = statement.column_count();
        let entries = statement
            .query_map([], |row| {
                (0..columns)
                    .map(|column| row.get(column))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        (
            serde_json::to_string(&self.store.scan("s").unwrap()).unwrap(),
            entries,
        )
    }
}

fn query() -> EntryQuery {
    EntryQuery {
        scan_id: "s".into(),
        limit: 200,
        ..Default::default()
    }
}

#[test]
fn old_unknown_rows_are_filtered_before_paging_without_rewriting_the_snapshot() {
    let parent = format!("{ROOT}\\Cache");
    let files: Vec<_> = (0..270)
        .map(|i| file(&format!("{parent}\\{i:03}.bin"), 1000 - i))
        .collect();
    let fixture = Fixture::new(&files);
    let original = fixture.raw_snapshot();
    let rules = RuleSet::test_rules(vec![rule(&parent)]);
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(&[], &policy);
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let cache = QueryCache::default();
    let mut q = query();
    q.parent = Some(parent);
    q.risk = Some("known".into());
    assert_eq!(fixture.store.query(&q).unwrap().total, 0);

    let first = cache
        .query(&fixture.store, &q, &classifier, "origin")
        .unwrap();
    assert_eq!(first.total, 270);
    assert_eq!(first.items.len(), 200);
    assert!(first
        .items
        .iter()
        .all(|file| file.assessment.owner.as_deref() == Some("Fixture app")));
    q.offset = 200;
    q.limit = 73;
    let second = cache
        .query(&fixture.store, &q, &classifier, "origin")
        .unwrap();
    assert_eq!(second.total, 270);
    assert_eq!(second.items.len(), 70);
    assert!(first
        .items
        .iter()
        .all(|first| second.items.iter().all(|second| first.id != second.id)));
    assert_eq!(cache.build_count(), 1);
    q.offset = 900;
    let empty = cache
        .query(&fixture.store, &q, &classifier, "origin")
        .unwrap();
    assert_eq!(empty.total, 270);
    assert!(empty.items.is_empty());
    assert_eq!(cache.build_count(), 1);

    q.offset = 0;
    q.risk = Some("unknown".into());
    assert_eq!(
        cache
            .query(&fixture.store, &q, &classifier, "origin")
            .unwrap()
            .total,
        0
    );
    assert_eq!(fixture.raw_snapshot(), original);
}

#[test]
fn analysis_filters_page_current_ids_and_refresh_when_the_id_set_changes() {
    let files: Vec<_> = (0..270)
        .map(|i| file(&format!("{ROOT}\\{i:03}.bin"), 1000 - i))
        .collect();
    let fixture = Fixture::new(&files);
    let records = fixture.store.page_after("s", 0, false).unwrap();
    let analyzed_ids: BTreeSet<_> = records.iter().skip(240).map(|file| file.id).collect();
    let rules = RuleSet::test_rules(Vec::new());
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(&[], &policy);
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let cache = QueryCache::default();
    let mut q = query();
    q.analysis_status = "analyzed".into();
    q.limit = 20;
    let analysis = AnalysisFilter::new(&q.analysis_status, analyzed_ids.clone()).unwrap();

    let first = cache
        .query_with_analysis(&fixture.store, &q, &classifier, "origin", analysis.as_ref())
        .unwrap();
    assert_eq!(first.total, 30);
    assert_eq!(first.items.len(), 20);
    assert_eq!(first.items[0].name, "240.bin");
    assert_eq!(first.items[19].name, "259.bin");
    q.offset = 20;
    let second = cache
        .query_with_analysis(&fixture.store, &q, &classifier, "origin", analysis.as_ref())
        .unwrap();
    assert_eq!(second.total, 30);
    assert_eq!(second.items.len(), 10);
    assert_eq!(second.items[0].name, "260.bin");
    q.offset = 100;
    let beyond = cache
        .query_with_analysis(&fixture.store, &q, &classifier, "origin", analysis.as_ref())
        .unwrap();
    assert_eq!(beyond.total, 30);
    assert!(beyond.items.is_empty());
    assert_eq!(cache.build_count(), 1);

    q.offset = 0;
    let changed = AnalysisFilter::new(
        &q.analysis_status,
        records.iter().take(30).map(|file| file.id).collect(),
    )
    .unwrap();
    let refreshed = cache
        .query_with_analysis(&fixture.store, &q, &classifier, "origin", changed.as_ref())
        .unwrap();
    assert_eq!(refreshed.total, 30);
    assert_eq!(refreshed.items[0].name, "000.bin");
    assert_eq!(cache.build_count(), 2);

    q.analysis_status = "unanalyzed".into();
    let inverse = AnalysisFilter::new(&q.analysis_status, analyzed_ids.clone()).unwrap();
    let remaining = cache
        .query_with_analysis(&fixture.store, &q, &classifier, "origin", inverse.as_ref())
        .unwrap();
    assert_eq!(remaining.total, 240);
    assert_eq!(remaining.items.len(), 20);
    assert!(remaining
        .items
        .iter()
        .all(|file| !analyzed_ids.contains(&file.id)));
}

#[test]
fn analysis_filters_combine_with_current_risk_and_search_and_require_valid_context() {
    let parent = format!("{ROOT}\\Cache");
    let fixture = Fixture::new(&[
        file(&format!("{parent}\\match-analyzed.bin"), 100),
        file(&format!("{parent}\\other.bin"), 200),
        file(&format!("{ROOT}\\Personal\\match.bin"), 300),
        file(&format!("{parent}\\match-pending.bin"), 400),
    ]);
    let records = fixture.store.page_after("s", 0, false).unwrap();
    let analyzed_ids: BTreeSet<_> = records.iter().take(3).map(|file| file.id).collect();
    let rules = RuleSet::test_rules(vec![rule(&parent)]);
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(&[], &policy);
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let cache = QueryCache::default();
    let mut q = query();
    q.risk = Some("known".into());
    q.search = Some("MATCH".into());
    for (status, expected) in [
        ("analyzed", "match-analyzed.bin"),
        ("unanalyzed", "match-pending.bin"),
    ] {
        q.analysis_status = status.into();
        let analysis = AnalysisFilter::new(status, analyzed_ids.clone()).unwrap();
        let result = cache
            .query_with_analysis(&fixture.store, &q, &classifier, "origin", analysis.as_ref())
            .unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].name, expected);
        assert_eq!(
            result.items[0].assessment.owner.as_deref(),
            Some("Fixture app")
        );
    }
    for (status, expected) in [("analyzed", 0), ("unanalyzed", 2), ("", 2)] {
        q.analysis_status = status.into();
        let analysis = AnalysisFilter::new(status, BTreeSet::new()).unwrap();
        let result = cache
            .query_with_analysis(&fixture.store, &q, &classifier, "origin", analysis.as_ref())
            .unwrap();
        assert_eq!(result.total, expected, "status: {status}");
    }

    q.analysis_status = "analyzed".into();
    assert!(fixture.store.query(&q).is_err());
    assert!(cache
        .query(&fixture.store, &q, &classifier, "origin")
        .is_err());
    let inverse = AnalysisFilter::new("unanalyzed", analyzed_ids).unwrap();
    assert!(cache
        .query_with_analysis(&fixture.store, &q, &classifier, "origin", inverse.as_ref())
        .is_err());
}

#[test]
fn cached_filters_follow_labels_rule_changes_and_origin_keys() {
    let path = format!("{ROOT}\\cache.bin");
    let fixture = Fixture::new(&[file(&path, 100)]);
    let rules = RuleSet::test_rules(Vec::new());
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(&[], &policy);
    let cache = QueryCache::default();
    let mut q = query();
    q.risk = Some("known".into());
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    assert_eq!(
        cache
            .query(&fixture.store, &q, &classifier, "origin-1")
            .unwrap()
            .total,
        0
    );

    let mut settings = Settings::default();
    settings.labels.insert(path.clone(), "My app".into());
    let labelled = SafetyPolicy::new(settings);
    let classifier = Classifier::new(&fixture.scan, &rules, &labelled, &apps);
    let page = cache
        .query(&fixture.store, &q, &classifier, "origin-1")
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].assessment.owner.as_deref(), Some("My app"));

    let mut rules = RuleSet::test_rules(vec![rule(&path)]);
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let page = cache
        .query(&fixture.store, &q, &classifier, "origin-1")
        .unwrap();
    assert_eq!(
        page.items[0].assessment.rule_id.as_deref(),
        Some("fixture-cache")
    );
    let before = cache.build_count();
    rules.version = "updated-rule-pack".into();
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    cache
        .query(&fixture.store, &q, &classifier, "origin-1")
        .unwrap();
    cache
        .query(&fixture.store, &q, &classifier, "origin-2")
        .unwrap();
    assert_eq!(cache.build_count(), before + 2);
}

#[test]
fn filtered_pages_and_fast_pages_share_current_protection_and_descendant_checks() {
    let mut root = file(ROOT, 100);
    root.is_dir = true;
    let mut protected = file(&format!("{ROOT}\\Protected"), 90);
    protected.is_dir = true;
    let mut incomplete = file(&format!("{ROOT}\\Incomplete"), 80);
    incomplete.is_dir = true;
    incomplete.complete = false;
    let mut blocked = file(&format!("{ROOT}\\Blocked"), 70);
    blocked.is_dir = true;
    blocked.has_blocked_children = true;
    let mut installed_parent = file(&format!("{ROOT}\\InstalledParent"), 60);
    installed_parent.is_dir = true;
    let ordinary = file(&format!("{ROOT}\\ordinary.bin"), 50);
    let fixture = Fixture::new(&[
        root,
        protected,
        incomplete,
        blocked,
        installed_parent,
        ordinary,
    ]);
    let original = fixture.raw_snapshot();
    let mut settings = Settings::default();
    settings
        .protected_paths
        .push(format!("{ROOT}\\Protected\\private"));
    let policy = SafetyPolicy::new(settings);
    let apps = ApplicationIndex::new(
        &[InstalledApp {
            id: "app".into(),
            name: "Installed app".into(),
            publisher: String::new(),
            install_location: format!("{ROOT}\\InstalledParent\\App"),
            source: "test".into(),
            last_used: None,
        }],
        &policy,
    );
    let rules = RuleSet::test_rules(Vec::new());
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let cache = QueryCache::default();
    let mut q = query();
    q.parent = Some(ROOT.into());
    let all = cache
        .query(&fixture.store, &q, &classifier, "origin")
        .unwrap();
    assert_eq!(cache.build_count(), 0);
    q.risk = Some("protected".into());
    let protected = cache
        .query(&fixture.store, &q, &classifier, "origin")
        .unwrap();
    assert_eq!(protected.total, 4);
    for file in &protected.items {
        let visible = all.items.iter().find(|other| other.id == file.id).unwrap();
        assert_eq!(
            serde_json::to_value(&visible.assessment).unwrap(),
            serde_json::to_value(&file.assessment).unwrap()
        );
        assert!(file.assessment.protected_reason.is_some());
    }
    q.parent = None;
    assert_eq!(
        cache
            .query(&fixture.store, &q, &classifier, "origin")
            .unwrap()
            .total,
        5
    );
    q.risk = Some("unknown".into());
    let unknown = cache
        .query(&fixture.store, &q, &classifier, "origin")
        .unwrap();
    assert_eq!(unknown.total, 1);
    assert_eq!(unknown.items[0].name, "ordinary.bin");
    assert_eq!(fixture.raw_snapshot(), original);
}

#[test]
fn structure_and_current_owner_category_and_uncertainty_filters_combine() {
    let mut directory = file(&format!("{ROOT}\\Cache\\IssueFolder"), 300);
    directory.is_dir = true;
    directory.issue = Some("fixture issue".into());
    let mut small = file(&format!("{ROOT}\\Cache\\IssueSmall"), 99);
    small.is_dir = true;
    small.issue = Some("fixture issue".into());
    let mut other_parent = directory.clone();
    other_parent.path = format!("{ROOT}\\Other\\IssueFolder");
    other_parent.parent = format!("{ROOT}\\Other");
    let fixture = Fixture::new(&[
        directory,
        small,
        other_parent,
        file(&format!("{ROOT}\\Cache\\regular.bin"), 400),
    ]);
    let rules = RuleSet::test_rules(vec![rule(&format!("{ROOT}\\Cache"))]);
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(&[], &policy);
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let cache = QueryCache::default();
    let mut q = query();
    q.parent = Some(format!("{ROOT}/CACHE"));
    q.search = Some("ISSUE".into());
    q.directories_only = true;
    q.issues_only = true;
    q.minimum_bytes = 100;
    q.category = Some("cache".into());
    q.owner = Some("Fixture app".into());
    q.sort = Some("name".into());
    let result = cache
        .query(&fixture.store, &q, &classifier, "origin")
        .unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].name, "IssueFolder");
    q.uncertain_only = true;
    assert_eq!(
        cache
            .query(&fixture.store, &q, &classifier, "origin")
            .unwrap()
            .total,
        0
    );
}

#[test]
fn groups_use_current_classification_and_file_allocations_without_directory_double_counting() {
    let parent = format!("{ROOT}\\Cache");
    let mut directory = file(&parent, 50_000);
    directory.is_dir = true;
    let fixture = Fixture::new(&[
        directory,
        file(&format!("{parent}\\first.bin"), 200),
        file(&format!("{parent}\\second.bin"), 300),
    ]);
    let rules = RuleSet::test_rules(vec![rule(&parent)]);
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(&[], &policy);
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let cache = QueryCache::default();
    let groups = cache
        .groups(&fixture.store, &classifier, "origin", "owner")
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(
        (groups[0].name.as_str(), groups[0].count, groups[0].bytes),
        ("Fixture app", 2, 502)
    );
    cache
        .groups(&fixture.store, &classifier, "origin", "owner")
        .unwrap();
    assert_eq!(cache.build_count(), 1);
    assert_eq!(
        cache
            .groups(&fixture.store, &classifier, "origin", "category")
            .unwrap()[0]
            .name,
        "cache"
    );
}

#[test]
fn next_age_threshold_includes_currently_excluded_matches() {
    let now = chrono::Utc::now().timestamp();
    let path = format!("{ROOT}\\recent.bin");
    let mut recent = file(&path, 100);
    recent.modified = now;
    recent.latest_change = now;
    let fixture = Fixture::new(&[recent.clone()]);
    let rules = RuleSet::test_rules(vec![rule(&path)]);
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(&[], &policy);
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    assert_eq!(
        classifier.next_change(&recent, now),
        Some(now + 14 * 86_400 + 1)
    );
    let mut q = query();
    q.risk = Some("low".into());
    let cache = QueryCache::default();
    assert_eq!(
        cache
            .query(&fixture.store, &q, &classifier, "origin")
            .unwrap()
            .total,
        0
    );
    assert_eq!(cache.next_expiry(), Some(now + 14 * 86_400 + 1));
    recent.modified = 0;
    recent.latest_change = 0;
    assert_eq!(classifier.next_change(&recent, now), None);
}

#[test]
fn sort_changes_have_separate_views_and_name_sort_uses_normalized_paths() {
    let mut first = file(&format!("{ROOT}\\z.bin"), 100);
    first.latest_change = 1_800_000_000;
    let second = file(&format!("{ROOT}\\B.bin"), 100);
    let third = file(&format!("{ROOT}\\a.bin"), 100);
    let fixture = Fixture::new(&[first, second, third]);
    let rules = RuleSet::test_rules(Vec::new());
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(&[], &policy);
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let cache = QueryCache::default();
    let mut q = query();
    q.risk = Some("unknown".into());
    q.sort = Some("name".into());
    let named = cache
        .query(&fixture.store, &q, &classifier, "origin")
        .unwrap();
    assert_eq!(
        named
            .items
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>(),
        vec!["a.bin", "B.bin", "z.bin"]
    );
    for (sort, expected, builds) in [
        ("activity", ["z.bin", "B.bin", "a.bin"], 2),
        ("activity_desc", ["z.bin", "B.bin", "a.bin"], 2),
        ("activity_asc", ["B.bin", "a.bin", "z.bin"], 3),
    ] {
        q.sort = Some(sort.into());
        let classified = cache
            .query(&fixture.store, &q, &classifier, "origin")
            .unwrap();
        assert_eq!(
            classified
                .items
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>(),
            expected,
            "classified sort: {sort}"
        );
        let mut fast = q.clone();
        fast.risk = None;
        let ordinary = cache
            .query(&fixture.store, &fast, &classifier, "origin")
            .unwrap();
        assert_eq!(
            ordinary
                .items
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>(),
            expected,
            "ordinary sort: {sort}"
        );
        assert_eq!(cache.build_count(), builds);
    }
}

#[test]
fn unfinished_scan_does_not_cache_a_partial_view() {
    let mut fixture = Fixture::new(&[file(&format!("{ROOT}\\first.bin"), 100)]);
    fixture.scan.status = "scanning".into();
    fixture.store.save_scan(&fixture.scan).unwrap();
    let rules = RuleSet::test_rules(Vec::new());
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(&[], &policy);
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let cache = QueryCache::default();
    let mut q = query();
    q.risk = Some("unknown".into());
    assert_eq!(
        cache
            .query(&fixture.store, &q, &classifier, "origin")
            .unwrap()
            .total,
        1
    );
    Store::insert_batch(
        &mut fixture.store.connection().unwrap(),
        "s",
        &[file(&format!("{ROOT}\\second.bin"), 200)],
    )
    .unwrap();
    assert_eq!(
        cache
            .query(&fixture.store, &q, &classifier, "origin")
            .unwrap()
            .total,
        2
    );
    assert_eq!(cache.build_count(), 0);
}

#[test]
fn installation_protection_survives_removal_of_a_vendor_origin() {
    let parent = format!("{ROOT}\\Container");
    let install_root = format!("{parent}\\Epic Games");
    let mut directory = file(&parent, 100);
    directory.is_dir = true;
    let fixture = Fixture::new(&[directory]);
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::new(
        &[InstalledApp {
            id: "epic".into(),
            name: "Epic Launcher".into(),
            publisher: String::new(),
            install_location: install_root.clone(),
            source: "fixture".into(),
            last_used: None,
        }],
        &policy,
    );
    assert!(apps
        .origin(&cleaner_platform::normalize(&install_root))
        .is_none());
    assert!(apps
        .installed_reason(&file(&format!("{install_root}\\program.exe"), 100))
        .is_some());
    let rules = RuleSet::test_rules(Vec::new());
    let classifier = Classifier::new(&fixture.scan, &rules, &policy, &apps);
    let mut q = query();
    q.parent = Some(ROOT.into());
    q.risk = Some("protected".into());
    let result = QueryCache::default()
        .query(&fixture.store, &q, &classifier, "origin")
        .unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].path, parent);
    assert!(result.items[0].assessment.owner.is_none());
    assert!(!result.items[0].has_blocked_children);
}

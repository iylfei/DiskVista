use super::*;

fn file(
    id: i64,
    path: &str,
    size: u64,
    directory: bool,
    rule: Option<&str>,
    risk: &str,
) -> FileRecord {
    FileRecord {
        id,
        path: path.into(),
        parent: path.rsplit_once('\\').map(|(p, _)| p).unwrap_or("").into(),
        name: path.rsplit('\\').next().unwrap().into(),
        logical_bytes: size,
        allocated_bytes: Some(size),
        is_dir: directory,
        complete: true,
        assessment: Assessment {
            rule_id: rule.map(String::from),
            risk: risk.into(),
            purpose: "缓存副本".into(),
            consequence: "可能需要重新下载".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
struct Fixture {
    index: SuggestionIndex,
    _dir: tempfile::TempDir,
}
impl std::ops::Deref for Fixture {
    type Target = SuggestionIndex;
    fn deref(&self) -> &Self::Target {
        &self.index
    }
}
fn index(entries: Vec<FileRecord>) -> Fixture {
    index_with_budget(entries, 8 * 1024 * 1024)
}
fn index_with_budget(entries: Vec<FileRecord>, budget: usize) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("index.db")).unwrap();
    store
        .save_scan(&Scan {
            id: "s".into(),
            root: "D:\\".into(),
            started: 100,
            finished: Some(110),
            status: "complete".into(),
            ..Default::default()
        })
        .unwrap();
    Store::insert_batch(&mut store.connection().unwrap(), "s", &entries).unwrap();
    let index = SuggestionIndex::from_records_with_budget(
        &store,
        "s",
        HashMap::from([("cache".into(), "应用缓存".into())]),
        entries.into_iter().map(Ok),
        budget,
    )
    .unwrap();
    Fixture { index, _dir: dir }
}
fn query() -> SuggestionQuery {
    SuggestionQuery {
        limit: 100,
        ..Default::default()
    }
}

#[test]
fn spilled_index_matches_memory_for_filters_pages_and_recycled_paths() {
    let mut records = vec![
        file(1, r"D:\Cache", 500, true, Some("cache"), "review"),
        file(2, r"D:\Cache\old", 200, false, Some("cache"), "low"),
        file(3, r"D:\Cache\nested", 300, true, Some("other"), "low"),
        file(
            4,
            r"D:\Cache\nested\keep",
            100,
            false,
            Some("cache"),
            "protected",
        ),
        file(5, r"D:\Cache-copy", 100, true, Some("cache"), "low"),
        file(
            6,
            r"\\?\UNC\server\share\gone\file",
            100,
            false,
            Some("cache"),
            "low",
        ),
    ];
    records.extend((7..=550).map(|id| {
        let mut f = file(
            id,
            &format!(r"D:\other\{id:04}"),
            id as u64,
            false,
            if id % 3 == 0 { None } else { Some("cache") },
            if id % 7 == 0 { "protected" } else { "low" },
        );
        f.latest_change = id % 13;
        f
    }));
    let memory = index(records.clone());
    let disk = SuggestionIndex::from_records_with_budget(
        &memory.store,
        "s",
        HashMap::from([("cache".into(), "应用缓存".into())]),
        records.into_iter().map(Ok),
        1,
    )
    .unwrap();
    assert!(disk.disk.is_some());
    assert!(disk.entries.is_empty() && disk.assessments.is_empty());
    for recycled in [false, true] {
        if recycled {
            for (id, path, is_dir) in [
                ("child", r"D:\Cache\old", false),
                ("unc", r"\\server\share\gone", true),
            ] {
                memory
                    .store
                    .add_history_for_scan(&history_item(id, path, "recycled", is_dir), "s")
                    .unwrap();
            }
        }
        for risk in ["", "low", "known", "unknown", "protected"] {
            for sort in ["size", "name", "activity_asc", "activity_desc"] {
                for status in ["", "analyzed", "unanalyzed"] {
                    let filter =
                        AnalysisFilter::new(status, [2, 4, 7, 9, 12, 15].into_iter().collect())
                            .unwrap();
                    for offset in [0, 20] {
                        let q = SuggestionQuery {
                            risk: risk.into(),
                            sort: sort.into(),
                            analysis_status: status.into(),
                            offset,
                            limit: 20,
                            ..query()
                        };
                        let expected = page_with_analysis(&memory, &q, filter.as_ref()).unwrap();
                        let actual = page_with_analysis(&disk, &q, filter.as_ref()).unwrap();
                        assert_eq!(
                            actual.total, expected.total,
                            "risk={risk} sort={sort} status={status} recycled={recycled}"
                        );
                        assert_eq!(
                            serde_json::to_value(actual.groups).unwrap(),
                            serde_json::to_value(expected.groups).unwrap()
                        );
                        assert_eq!(
                            serde_json::to_value(actual.items).unwrap(),
                            serde_json::to_value(expected.items).unwrap()
                        );
                    }
                }
            }
        }
        for search in ["", "00"] {
            let q = SuggestionQuery {
                group: Some("rule:cache".into()),
                search: search.into(),
                ..query()
            };
            assert_eq!(
                serde_json::to_value(selection(&disk, &q).unwrap()).unwrap(),
                serde_json::to_value(selection(&memory, &q).unwrap()).unwrap()
            );
        }
    }
}

#[test]
fn groups_collapse_nested_files_and_preserve_size_and_purpose() {
    let data = index(vec![
        file(1, "D:\\Cache", 300_000_000, true, Some("cache"), "review"),
        file(
            2,
            "D:\\Cache\\old",
            150_000_000,
            false,
            Some("cache"),
            "low",
        ),
        file(
            3,
            "D:\\Cache\\recent",
            150_000_000,
            false,
            Some("cache"),
            "review",
        ),
        file(
            4,
            "D:\\Downloads\\video.mp4",
            200_000_000,
            false,
            None,
            "review",
        ),
    ]);
    let result = page(&data, &query()).unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(result.groups[0].id, "large-files");
    assert_eq!(result.groups[1].name, "应用缓存");
    assert_eq!(result.groups[1].occupied_bytes, 300_000_000);
    assert_eq!(result.groups[1].count, 1);
    assert_eq!(result.groups[1].consequence, "可能需要重新下载");
    let mut q = query();
    q.risk = "low".into();
    let low = page(&data, &q).unwrap();
    assert_eq!(low.items[0].id, 2);
    assert_eq!(low.groups[0].occupied_bytes, 150_000_000);
    q.risk.clear();
    q.search = "recent".into();
    assert_eq!(page(&data, &q).unwrap().items[0].id, 3);
}

#[test]
fn current_rules_find_small_files_in_old_snapshots_without_rewriting_them() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("index.db")).unwrap();
    let root = "D:\\RuleScope";
    store
        .save_scan(&Scan {
            id: "s".into(),
            root: root.into(),
            status: "complete".into(),
            ..Default::default()
        })
        .unwrap();
    let records = vec![
        file(0, root, 0, true, None, "review"),
        file(0, "D:\\RuleScope\\Cache", 10, true, None, "review"),
        file(
            0,
            "D:\\RuleScope\\Cache\\small.bin",
            10,
            false,
            None,
            "review",
        ),
        file(
            0,
            "D:\\RuleScope\\Profiles\\Default\\Cache",
            20,
            true,
            None,
            "review",
        ),
        file(
            0,
            "D:\\RuleScope\\Profiles\\Default\\Cache\\small.bin",
            20,
            false,
            None,
            "review",
        ),
        file(
            0,
            "D:\\RuleScope\\Cache-copy\\personal.bin",
            30,
            false,
            None,
            "review",
        ),
        file(
            0,
            "D:\\RuleScope\\Profiles\\Default\\Private\\personal.txt",
            3,
            false,
            None,
            "review",
        ),
        file(
            0,
            "D:\\RuleScope\\movie.mkv",
            200_000_000,
            false,
            None,
            "review",
        ),
        file(
            0,
            "D:\\RuleScope\\retired\\old.bin",
            5,
            false,
            Some("retired-rule"),
            "review",
        ),
    ];
    Store::insert_batch(&mut store.connection().unwrap(), "s", &records).unwrap();
    let original = store
        .by_path("s", "D:\\RuleScope\\Cache\\small.bin")
        .unwrap();
    let template = RuleSet::load(false)
        .unwrap()
        .rules
        .into_iter()
        .find(|r| r.id == "vscode-cache")
        .unwrap();
    let mut fixed = template.clone();
    fixed.id = "current-fixed".into();
    fixed.root = "D:\\RuleScope\\Cache".into();
    let mut wildcard = template;
    wildcard.id = "current-wildcard".into();
    wildcard.root = "D:\\RuleScope\\Profiles\\*\\Cache".into();
    let active = RuleSet::test_rules(vec![fixed, wildcard]);
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::for_scan(&store, "s", &policy).unwrap();
    let index = build_with_rules(&store, "s", &policy, &apps, &active).unwrap();
    let result = page(&index, &query()).unwrap();
    assert_eq!(result.total, 3);
    assert!(result
        .items
        .iter()
        .any(|f| f.path == "D:\\RuleScope\\Cache"));
    assert!(result
        .items
        .iter()
        .any(|f| f.path == "D:\\RuleScope\\Profiles\\Default\\Cache"));
    assert!(result
        .items
        .iter()
        .any(|f| f.path == "D:\\RuleScope\\movie.mkv"));
    let disabled = RuleSet::test_rules(vec![]);
    let without_rules = build_with_rules(&store, "s", &policy, &apps, &disabled).unwrap();
    let unrecognized = page(&without_rules, &query()).unwrap();
    assert_eq!(unrecognized.total, 1);
    assert_eq!(unrecognized.items[0].path, "D:\\RuleScope\\movie.mkv");
    let unchanged = store.by_path("s", &original.path).unwrap();
    assert_eq!(
        serde_json::to_value(original).unwrap(),
        serde_json::to_value(unchanged).unwrap()
    );
}

#[test]
fn suggestions_expire_at_the_first_rule_age_transition_or_clock_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("index.db")).unwrap();
    store
        .save_scan(&Scan {
            id: "s".into(),
            root: "D:\\AgeScope".into(),
            status: "complete".into(),
            ..Default::default()
        })
        .unwrap();
    let now = chrono::Utc::now().timestamp();
    let mut recent = file(
        0,
        "D:\\AgeScope\\Cache\\recent.bin",
        10,
        false,
        None,
        "review",
    );
    recent.modified = now;
    recent.latest_change = now;
    let mut older = file(
        0,
        "D:\\AgeScope\\Cache\\older.bin",
        20,
        false,
        None,
        "review",
    );
    older.modified = now - 43_200;
    older.latest_change = now - 43_200;
    Store::insert_batch(&mut store.connection().unwrap(), "s", &[recent, older]).unwrap();
    let mut rule = RuleSet::load(false)
        .unwrap()
        .rules
        .into_iter()
        .find(|r| r.id == "vscode-cache")
        .unwrap();
    rule.root = "D:\\AgeScope\\Cache".into();
    rule.age_days = 1;
    let rules = RuleSet::test_rules(vec![rule]);
    let policy = SafetyPolicy::new(Default::default());
    let apps = ApplicationIndex::for_scan(&store, "s", &policy).unwrap();
    let index = build_with_rules(&store, "s", &policy, &apps, &rules).unwrap();
    let result = page(&index, &query()).unwrap();
    assert_eq!(result.total, 2);
    assert!(result
        .items
        .iter()
        .all(|file| file.assessment.risk == "review"));
    let transition = now + 43_201;
    assert_eq!(index.valid_until, Some(transition));
    assert!(index.valid_at(transition - 1));
    assert!(!index.valid_at(transition));
    assert!(!index.valid_at(index.started - 1));
}

#[test]
fn group_selection_is_explicit_and_never_includes_protected_or_unknown_files() {
    let data = index(vec![
        file(1, "D:\\Cache\\safe", 60, false, Some("cache"), "low"),
        file(
            2,
            "D:\\Cache\\protected",
            60,
            false,
            Some("cache"),
            "protected",
        ),
        file(3, "D:\\personal.dat", 200_000_000, false, None, "review"),
    ]);
    assert_eq!(page(&data, &query()).unwrap().total, 2);
    assert!(selection(&data, &query()).is_err());
    let mut q = query();
    q.group = Some("rule:cache".into());
    assert_eq!(
        selection(&data, &q)
            .unwrap()
            .iter()
            .map(|f| f.id)
            .collect::<Vec<_>>(),
        vec![1]
    );
    q.risk = "protected".into();
    assert_eq!(page(&data, &q).unwrap().items[0].id, 2);
    assert!(selection(&data, &q).unwrap().is_empty());
    q.risk.clear();
    q.group = Some("large-files".into());
    assert!(selection(&data, &q).is_err());
}

#[test]
fn analysis_filters_page_before_slicing_and_keep_group_totals_and_cache_current() {
    let data = index(
        (1..=150)
            .map(|id| {
                file(
                    id,
                    &format!("D:\\Cache\\{id:03}"),
                    1,
                    false,
                    Some("cache"),
                    "low",
                )
            })
            .collect(),
    );
    let mut q = query();
    q.analysis_status = "analyzed".into();
    q.limit = 20;
    let analysis = AnalysisFilter::new("analyzed", (121..=150).collect()).unwrap();
    let first = page_with_analysis(&data, &q, analysis.as_ref()).unwrap();
    assert_eq!(first.total, 30);
    assert_eq!(
        first.items.iter().map(|file| file.id).collect::<Vec<_>>(),
        (121..=140).collect::<Vec<_>>()
    );
    assert_eq!(first.groups[0].count, 30);
    assert_eq!(first.groups[0].occupied_bytes, 30);
    q.offset = 20;
    let second = page_with_analysis(&data, &q, analysis.as_ref()).unwrap();
    assert_eq!(second.total, 30);
    assert_eq!(
        second.items.iter().map(|file| file.id).collect::<Vec<_>>(),
        (141..=150).collect::<Vec<_>>()
    );
    assert_eq!(second.groups[0].count, 30);
    assert_eq!(data.views.lock().unwrap().len(), 1);

    q.offset = 0;
    let changed = AnalysisFilter::new("analyzed", (1..=30).collect()).unwrap();
    let refreshed = page_with_analysis(&data, &q, changed.as_ref()).unwrap();
    assert_eq!(refreshed.total, 30);
    assert_eq!(refreshed.items[0].id, 1);
    assert_eq!(data.views.lock().unwrap().len(), 2);
    q.analysis_status = "unanalyzed".into();
    let inverse = AnalysisFilter::new("unanalyzed", (121..=150).collect()).unwrap();
    let remaining = page_with_analysis(&data, &q, inverse.as_ref()).unwrap();
    assert_eq!(remaining.total, 120);
    assert_eq!(
        remaining
            .items
            .iter()
            .map(|file| file.id)
            .collect::<Vec<_>>(),
        (1..=20).collect::<Vec<_>>()
    );
    assert_eq!(remaining.groups[0].count, 120);
    assert_eq!(remaining.groups[0].occupied_bytes, 120);

    for (status, expected) in [("analyzed", 0), ("unanalyzed", 150), ("", 150)] {
        q.analysis_status = status.into();
        let empty = AnalysisFilter::new(status, Default::default()).unwrap();
        let result = page_with_analysis(&data, &q, empty.as_ref()).unwrap();
        assert_eq!(result.total, expected, "status: {status}");
        assert_eq!(
            result.groups.iter().map(|group| group.count).sum::<usize>(),
            expected
        );
        assert_eq!(result.items.len(), expected.min(20));
    }
}

#[test]
fn analyzed_children_remain_visible_and_selection_preserves_risk_and_search_filters() {
    let data = index(vec![
        file(1, "D:\\Cache", 190, true, Some("cache"), "review"),
        file(
            2,
            "D:\\Cache\\match-analyzed",
            40,
            false,
            Some("cache"),
            "low",
        ),
        file(
            3,
            "D:\\Cache\\match-pending",
            60,
            false,
            Some("cache"),
            "low",
        ),
        file(
            4,
            "D:\\Cache\\match-protected",
            80,
            false,
            Some("cache"),
            "protected",
        ),
        file(
            5,
            "D:\\Personal\\match.bin",
            200_000_000,
            false,
            None,
            "review",
        ),
        file(6, "D:\\Cache\\other", 10, false, Some("cache"), "low"),
    ]);
    let ids = [2, 4, 5, 6].into_iter().collect();
    let analysis = AnalysisFilter::new("analyzed", ids).unwrap();
    let mut q = query();
    q.analysis_status = "analyzed".into();
    q.group = Some("rule:cache".into());
    let result = page_with_analysis(&data, &q, analysis.as_ref()).unwrap();
    assert_eq!(
        result.items.iter().map(|file| file.id).collect::<Vec<_>>(),
        vec![2, 6]
    );
    let cache_group = result
        .groups
        .iter()
        .find(|group| group.id == "rule:cache")
        .unwrap();
    assert_eq!((cache_group.count, cache_group.occupied_bytes), (2, 50));
    assert_eq!(result.groups[0].id, "large-files");
    assert_eq!(result.groups[0].count, 1);

    q.offset = 99;
    q.limit = 1;
    let beyond = page_with_analysis(&data, &q, analysis.as_ref()).unwrap();
    assert_eq!(beyond.total, 2);
    assert!(beyond.items.is_empty());
    assert_eq!(
        selection_with_analysis(&data, &q, analysis.as_ref())
            .unwrap()
            .iter()
            .map(|file| file.id)
            .collect::<Vec<_>>(),
        vec![2, 6]
    );
    q.offset = 0;
    q.risk = "low".into();
    q.search = "MATCH".into();
    for (status, expected) in [("analyzed", 2), ("unanalyzed", 3)] {
        q.analysis_status = status.into();
        let filter = AnalysisFilter::new(status, [2, 4, 5, 6].into_iter().collect()).unwrap();
        let filtered = page_with_analysis(&data, &q, filter.as_ref()).unwrap();
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.items[0].id, expected);
        assert_eq!(filtered.groups.len(), 1);
        assert_eq!(filtered.groups[0].count, 1);
        assert_eq!(
            selection_with_analysis(&data, &q, filter.as_ref()).unwrap()[0].id,
            expected
        );
    }
    q.analysis_status = "analyzed".into();
    q.risk = "protected".into();
    let protected = page_with_analysis(&data, &q, analysis.as_ref()).unwrap();
    assert_eq!(protected.total, 1);
    assert_eq!(protected.items[0].id, 4);
    assert!(selection_with_analysis(&data, &q, analysis.as_ref())
        .unwrap()
        .is_empty());
    assert!(page(&data, &q).is_err());
    assert!(selection(&data, &q).is_err());
    let mismatched = AnalysisFilter::new("unanalyzed", Default::default()).unwrap();
    assert!(page_with_analysis(&data, &q, mismatched.as_ref()).is_err());
    assert!(selection_with_analysis(&data, &q, mismatched.as_ref()).is_err());
}

#[test]
fn both_activity_directions_keep_ties_stable_across_pages() {
    let data = index(
        [30, 10, 20, 20]
            .into_iter()
            .enumerate()
            .map(|(i, changed)| {
                let id = i as i64 + 1;
                let mut record = file(
                    id,
                    &format!("D:\\Files\\{id}.bin"),
                    200_000_000,
                    false,
                    None,
                    "review",
                );
                record.latest_change = changed;
                record
            })
            .collect(),
    );
    let mut q = query();
    q.group = Some("large-files".into());
    q.limit = 2;
    for (sort, expected) in [
        ("activity", [1, 3, 4, 2]),
        ("activity_desc", [1, 3, 4, 2]),
        ("activity_asc", [2, 3, 4, 1]),
    ] {
        q.sort = sort.into();
        let mut ids = Vec::new();
        for offset in [0, 2] {
            q.offset = offset;
            let result = page(&data, &q).unwrap();
            assert_eq!(result.total, 4);
            ids.extend(result.items.into_iter().map(|file| file.id));
        }
        assert_eq!(ids, expected, "sort: {sort}");
    }
}

#[test]
fn large_batches_require_a_narrower_explicit_selection() {
    let data = index(
        (1..=501)
            .map(|id| {
                file(
                    id,
                    &format!("D:\\Cache\\{id}"),
                    1,
                    false,
                    Some("cache"),
                    "low",
                )
            })
            .collect(),
    );
    let mut q = query();
    q.group = Some("rule:cache".into());
    assert!(selection(&data, &q).is_err());
    q.offset = 100;
    q.limit = 100;
    assert_eq!(page(&data, &q).unwrap().items.len(), 100);
    assert_eq!(page(&data, &q).unwrap().total, 501);
    assert_eq!(data.views.lock().unwrap().len(), 1);
    q.offset = 0;
    let first = page(&data, &q).unwrap();
    q.offset = 100;
    let next = page(&data, &q).unwrap();
    assert!(first
        .items
        .iter()
        .all(|f| next.items.iter().all(|n| f.id != n.id)));
    assert_eq!(data.views.lock().unwrap().len(), 1);
    for search in ["1", "2", "3", "4", "5", "6"] {
        q.search = search.into();
        page(&data, &q).unwrap();
    }
    assert_eq!(data.views.lock().unwrap().len(), 4);
    q.search.clear();
    q.analysis_status = "analyzed".into();
    let analysis = AnalysisFilter::new("analyzed", (490..=501).collect()).unwrap();
    let filtered = selection_with_analysis(&data, &q, analysis.as_ref()).unwrap();
    assert_eq!(
        filtered.iter().map(|file| file.id).collect::<Vec<_>>(),
        (490..=501).collect::<Vec<_>>()
    );
}

#[test]
fn app_data_and_portable_origins_are_shared_without_expanding_cleanup_candidates() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("origins.sqlite")).unwrap();
    let root = std::env::var("USERPROFILE").unwrap();
    let local = std::env::var("LOCALAPPDATA").unwrap();
    let data = format!("{local}\\FixtureEditor\\model.bin");
    let portable = format!("{root}\\PortableFixture");
    let portable_file = format!("{portable}\\model.bin");
    store
        .save_scan(&Scan {
            id: "s".into(),
            root: root.clone(),
            status: "complete".into(),
            ..Default::default()
        })
        .unwrap();
    store
        .save_apps(
            "s",
            &[InstalledApp {
                id: "editor".into(),
                name: "FixtureEditor".into(),
                publisher: String::new(),
                install_location: "D:\\Installed\\FixtureEditor".into(),
                source: "Windows 卸载清单".into(),
                last_used: None,
            }],
        )
        .unwrap();
    Store::insert_batch(
        &mut store.connection().unwrap(),
        "s",
        &[
            file(1, &root, 400_001_000, true, None, "review"),
            file(2, &data, 200_000_000, false, None, "review"),
            file(3, &portable, 200_001_000, true, None, "review"),
            file(
                4,
                &format!("{portable}\\portable.exe"),
                1000,
                false,
                None,
                "review",
            ),
            file(5, &portable_file, 200_000_000, false, None, "review"),
            file(
                6,
                &format!("{local}\\FixtureEditor\\config.json"),
                64,
                false,
                None,
                "review",
            ),
        ],
    )
    .unwrap();
    let index = build(&store, "s").unwrap();
    let result = page(&index, &query()).unwrap();
    assert_eq!(result.total, 2);
    assert_eq!(
        result
            .items
            .iter()
            .find(|f| f.path == data)
            .unwrap()
            .assessment
            .owner
            .as_deref(),
        Some("FixtureEditor")
    );
    assert_eq!(
        result
            .items
            .iter()
            .find(|f| f.path == portable_file)
            .unwrap()
            .assessment
            .owner
            .as_deref(),
        Some("PortableFixture")
    );
    assert!(result
        .items
        .iter()
        .all(|f| f.assessment.risk == "review" && f.assessment.rule_id.is_none()));
}

#[test]
fn old_snapshots_respect_current_protection_and_exclude_unidentified_directories() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("suggestions.db")).unwrap();
    store
        .save_scan(&Scan {
            id: "s".into(),
            root: "D:\\fixture".into(),
            status: "complete".into(),
            ..Default::default()
        })
        .unwrap();
    Store::insert_batch(
        &mut store.connection().unwrap(),
        "s",
        &[
            file(1, "D:\\fixture\\folder", 300_000_000, true, None, "review"),
            file(
                2,
                "D:\\fixture\\folder\\large.bin",
                300_000_000,
                false,
                None,
                "review",
            ),
        ],
    )
    .unwrap();
    assert_eq!(
        page(&build(&store, "s").unwrap(), &query()).unwrap().total,
        1
    );
    store
        .put(
            "settings",
            &Settings {
                protected_paths: vec!["D:\\fixture\\folder".into()],
                ..Default::default()
            },
        )
        .unwrap();
    let fresh = build(&store, "s").unwrap();
    assert_eq!(page(&fresh, &query()).unwrap().total, 0);
    let mut q = query();
    q.risk = "protected".into();
    assert_eq!(page(&fresh, &q).unwrap().total, 1);
    assert_eq!(
        page(&fresh, &q).unwrap().items[0].assessment.risk,
        "protected"
    );

    let cache = format!(
        "{}\\CleanerSuggestionParent",
        std::env::var("TEMP").unwrap()
    );
    let child = format!("{cache}\\keep.tmp");
    store
        .put(
            "settings",
            &Settings {
                protected_paths: vec![child],
                ..Default::default()
            },
        )
        .unwrap();
    Store::insert_batch(
        &mut store.connection().unwrap(),
        "s",
        &[file(0, &cache, 100, true, Some("user-temp"), "low")],
    )
    .unwrap();
    let fresh = build(&store, "s").unwrap();
    assert!(!page(&fresh, &query())
        .unwrap()
        .items
        .iter()
        .any(|file| file.path == cache));
    assert!(page(&fresh, &q)
        .unwrap()
        .items
        .iter()
        .any(|file| file.path == cache));
}

fn history_item(id: &str, path: &str, status: &str, is_dir: bool) -> HistoryItem {
    HistoryItem {
        id: id.into(),
        batch_id: "cleanup".into(),
        path: path.into(),
        bytes: 100,
        time: 120,
        status: status.into(),
        message: String::new(),
        free_space_delta: 0,
        snapshot: Some(HistoryEntrySnapshot {
            name: path.rsplit('\\').next().unwrap_or("").into(),
            is_dir,
            owner: None,
            category: String::new(),
            rule_id: None,
        }),
    }
}

#[test]
fn partial_directory_cleanup_exposes_survivors_and_refreshes_cached_group_totals() {
    let data = index(vec![
        file(1, r"D:\Cache", 400, true, Some("cache"), "low"),
        file(2, r"D:\Cache\old.bin", 100, false, Some("cache"), "low"),
        file(3, r"D:\Cache\keep.bin", 50, false, Some("cache"), "low"),
        file(4, r"D:\Cache\Live", 250, true, Some("cache"), "low"),
        file(5, r"D:\Cache\Live\a.bin", 125, false, Some("cache"), "low"),
        file(6, r"D:\Cache\Live\b.bin", 125, false, Some("cache"), "low"),
        file(7, r"D:\Cache-copy", 90, true, Some("cache"), "low"),
    ]);
    let q = SuggestionQuery {
        group: Some("rule:cache".into()),
        ..query()
    };
    let before = page(&data, &q).unwrap();
    assert_eq!(before.total, 2);
    assert_eq!(before.groups[0].occupied_bytes, 490);
    let snapshot = serde_json::to_value(data.store.entry("s", 1).unwrap()).unwrap();
    for item in [
        history_item("deleted", r"d:/cache/OLD.bin", "recycled", false),
        history_item("failed", r"D:\Cache\keep.bin", "failed", false),
        history_item("skipped", r"D:\Cache\Live", "skipped", true),
    ] {
        data.store.add_history_for_scan(&item, "s").unwrap();
    }
    let after = page(&data, &q).unwrap();
    assert_eq!(after.total, 3);
    assert_eq!(after.groups[0].count, 3);
    assert_eq!(after.groups[0].occupied_bytes, 390);
    assert_eq!(
        selection(&data, &q)
            .unwrap()
            .iter()
            .map(|file| file.id)
            .collect::<Vec<_>>(),
        vec![4, 7, 3]
    );
    assert_eq!(
        serde_json::to_value(data.store.entry("s", 1).unwrap()).unwrap(),
        snapshot
    );

    data.store
        .add_history_for_scan(
            &history_item("directory", r"D:\Cache\Live", "recycled", true),
            "s",
        )
        .unwrap();
    let after_directory = page(&data, &q).unwrap();
    assert_eq!(after_directory.total, 2);
    assert_eq!(after_directory.groups[0].occupied_bytes, 140);
    assert_eq!(
        selection(&data, &q)
            .unwrap()
            .iter()
            .map(|file| file.id)
            .collect::<Vec<_>>(),
        vec![7, 3]
    );
}

#[test]
fn recycled_filter_precedes_paging_and_the_group_selection_limit() {
    for budget in [usize::MAX, 1] {
        let data = index_with_budget(
            (1..=607)
                .map(|id| {
                    file(
                        id,
                        &format!(r"D:\Files\{id}.bin"),
                        200_000_000 + id as u64,
                        false,
                        Some("cache"),
                        "low",
                    )
                })
                .collect(),
            budget,
        );
        let mut q = SuggestionQuery {
            group: Some("rule:cache".into()),
            limit: 2,
            ..query()
        };
        assert_eq!(page(&data, &q).unwrap().total, 607);
        assert!(selection(&data, &q).is_err());
        let mut connection = data.store.connection().unwrap();
        let transaction = connection.transaction().unwrap();
        for id in 1..=607 {
            let item = history_item(
                &format!("h-{id}"),
                &format!(r"D:\Files\{id}.bin"),
                if id <= 601 {
                    "recycled"
                } else if id == 607 {
                    "failed"
                } else {
                    "skipped"
                },
                false,
            );
            transaction
                .execute(
                    "INSERT INTO history VALUES(?1,?2,?3)",
                    rusqlite::params![item.id, item.time, serde_json::to_string(&item).unwrap()],
                )
                .unwrap();
        }
        transaction.commit().unwrap();
        let first = page(&data, &q).unwrap();
        assert_eq!(first.total, 6);
        assert_eq!(first.groups[0].count, 6);
        assert_eq!(
            first.groups[0].occupied_bytes,
            (602..=607).map(|id| 200_000_000 + id).sum::<u64>()
        );
        assert_eq!(
            first.items.iter().map(|file| file.id).collect::<Vec<_>>(),
            vec![607, 606]
        );
        q.offset = 2;
        assert_eq!(
            page(&data, &q)
                .unwrap()
                .items
                .iter()
                .map(|file| file.id)
                .collect::<Vec<_>>(),
            vec![605, 604]
        );
        assert_eq!(selection(&data, &q).unwrap().len(), 6);
    }
}

#[test]
fn later_scan_of_a_restored_path_is_not_hidden_by_older_recycle_history() {
    let data = index(vec![file(
        1,
        r"D:\Files\restored.bin",
        200_000_000,
        false,
        Some("cache"),
        "low",
    )]);
    data.store
        .add_history_for_scan(
            &history_item("h", r"D:\Files\restored.bin", "recycled", false),
            "s",
        )
        .unwrap();
    assert_eq!(page(&data, &query()).unwrap().total, 0);
    let new_scan = Scan {
        id: "new".into(),
        root: r"D:\Files".into(),
        started: 120,
        finished: Some(120),
        status: "complete".into(),
        ..Default::default()
    };
    data.store.save_scan(&new_scan).unwrap();
    let restored = file(
        0,
        r"D:\Files\restored.bin",
        200_000_000,
        false,
        Some("cache"),
        "low",
    );
    Store::insert_batch(&mut data.store.connection().unwrap(), "new", &[restored]).unwrap();
    let file = data.store.by_path("new", r"D:\Files\restored.bin").unwrap();
    let new_index =
        SuggestionIndex::from_records(&data.store, "new", HashMap::new(), [Ok(file)].into_iter())
            .unwrap();
    assert_eq!(page(&new_index, &query()).unwrap().total, 1);
}

#[test]
fn recycled_unc_aliases_are_matched_from_original_candidate_paths() {
    let data = index(vec![
        file(
            1,
            r"\\?\UNC\server\share\cache\old.bin",
            100,
            false,
            Some("cache"),
            "low",
        ),
        file(
            2,
            r"\\server\share\cache-copy\keep.bin",
            50,
            false,
            Some("cache"),
            "low",
        ),
    ]);
    data.store
        .add_history_for_scan(
            &history_item("unc", r"\\SERVER\Share\cache", "recycled", true),
            "s",
        )
        .unwrap();
    let result = page(&data, &query()).unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].id, 2);
}

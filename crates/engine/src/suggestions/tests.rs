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
fn index(entries: Vec<FileRecord>) -> SuggestionIndex {
    SuggestionIndex {
        entries,
        names: HashMap::from([("cache".into(), "应用缓存".into())]),
    }
}
fn query() -> SuggestionQuery {
    SuggestionQuery {
        limit: 100,
        ..Default::default()
    }
}

#[test]
fn groups_collapse_nested_files_and_preserve_size_and_purpose() {
    let data = index(vec![
        file(1, "D:\\Cache", 120, true, Some("cache"), "review"),
        file(2, "D:\\Cache\\old", 60, false, Some("cache"), "low"),
        file(3, "D:\\Cache\\recent", 60, false, Some("cache"), "review"),
        file(
            4,
            "D:\\Downloads\\video.mp4",
            200_000_000,
            false,
            None,
            "review",
        ),
    ]);
    let result = page(&data, &query());
    assert_eq!(result.total, 2);
    assert_eq!(result.groups[0].name, "应用缓存");
    assert_eq!(result.groups[0].occupied_bytes, 120);
    assert_eq!(result.groups[0].count, 1);
    assert_eq!(result.groups[0].consequence, "可能需要重新下载");
    assert_eq!(result.groups[1].id, "large-files");
    let mut q = query();
    q.risk = "low".into();
    let low = page(&data, &q);
    assert_eq!(low.items[0].id, 2);
    assert_eq!(low.groups[0].occupied_bytes, 60);
    q.risk.clear();
    q.search = "recent".into();
    assert_eq!(page(&data, &q).items[0].id, 3);
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
    assert_eq!(page(&data, &query()).total, 2);
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
    assert_eq!(page(&data, &q).items[0].id, 2);
    assert!(selection(&data, &q).unwrap().is_empty());
    q.risk.clear();
    q.group = Some("large-files".into());
    assert!(selection(&data, &q).is_err());
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
    assert_eq!(page(&data, &q).items.len(), 100);
    assert_eq!(page(&data, &q).total, 501);
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
    assert_eq!(page(&build(&store, "s").unwrap(), &query()).total, 1);
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
    assert_eq!(page(&fresh, &query()).total, 0);
    let mut q = query();
    q.risk = "protected".into();
    assert_eq!(page(&fresh, &q).total, 1);
    assert_eq!(page(&fresh, &q).items[0].assessment.risk, "protected");

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
        .items
        .iter()
        .any(|file| file.path == cache));
    assert!(page(&fresh, &q).items.iter().any(|file| file.path == cache));
}

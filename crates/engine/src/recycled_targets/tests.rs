use super::*;
use cleaner_domain::{FileRecord, HistoryEntrySnapshot, HistoryItem};

mod benchmark;

fn scan(id: &str, finished: i64) -> Scan {
    Scan {
        id: id.into(),
        root: "D:\\Fixture".into(),
        started: finished - 10,
        finished: Some(finished),
        status: "complete".into(),
        ..Default::default()
    }
}

fn history(id: &str, path: &str, time: i64, status: &str, is_dir: Option<bool>) -> HistoryItem {
    HistoryItem {
        id: id.into(),
        batch_id: "batch".into(),
        path: path.into(),
        bytes: 100,
        time,
        status: status.into(),
        message: String::new(),
        free_space_delta: 0,
        snapshot: is_dir.map(|is_dir| HistoryEntrySnapshot {
            name: path.rsplit('\\').next().unwrap_or("").into(),
            is_dir,
            owner: None,
            category: String::new(),
            rule_id: None,
        }),
    }
}

#[test]
fn only_confirmed_successes_after_the_snapshot_remove_paths_and_known_descendants() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("history.sqlite")).unwrap();
    let scan = scan("old", 100);
    for item in [
        history(
            "directory",
            r"D:\Fixture\Cache",
            101,
            "recycled",
            Some(true),
        ),
        history(
            "file",
            r"D:\Fixture\movie.bin",
            102,
            "recycled",
            Some(false),
        ),
        history(
            "failed",
            r"D:\Fixture\failed.bin",
            103,
            "failed",
            Some(false),
        ),
        history("skipped", r"D:\Fixture\skipped.bin", 104, "skipped", None),
        history("before", r"D:\Fixture\restored.bin", 99, "recycled", None),
        history("same-second", r"D:\Fixture\same.bin", 100, "recycled", None),
    ] {
        store.add_history(&item).unwrap();
    }
    let targets = RecycledTargets::load(&store, &scan).unwrap();
    assert!(targets.contains(r"\\?\d:\FIXTURE\CACHE\"));
    assert!(targets.contains(r"D:/Fixture/Cache/child/file.bin"));
    assert!(targets.contains(r"D:\Fixture\\cache\\child.bin"));
    assert!(targets.contains(r"D:\Fixture\movie.bin"));
    assert!(!targets.contains(r"D:\Fixture\Cache-copy\child.bin"));
    assert!(!targets.contains(r"D:\Fixture\movie.bin\child"));
    for name in ["failed", "skipped", "restored", "same"] {
        assert!(!targets.contains(&format!(r"D:\Fixture\{name}.bin")));
    }
    assert!(targets.affects_directory(r"D:\Fixture"));
    assert!(!targets.affects_directory(r"D:\Fixture-copy"));
    assert!(!targets.affects_directory(r"D:\Fixture\Cache-copy"));
}

#[test]
fn legacy_snapshots_never_infer_deleted_descendants_from_an_unknown_target_type() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("legacy.sqlite")).unwrap();
    store
        .add_history(&history(
            "legacy",
            r"D:\Fixture\unknown",
            101,
            "recycled",
            None,
        ))
        .unwrap();
    let targets = RecycledTargets::load(&store, &scan("old", 100)).unwrap();
    assert!(targets.contains(r"D:\Fixture\unknown"));
    assert!(!targets.contains(r"D:\Fixture\unknown\still-visible.bin"));
    assert!(targets.affects_directory(r"D:\Fixture"));
    assert!(!targets.affects_directory(r"D:\Fixture\unknown"));
}

#[test]
fn scoped_history_disambiguates_same_second_and_keeps_restored_new_scans_visible() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("scoped.sqlite")).unwrap();
    let item = history(
        "scoped",
        r"D:\Fixture\restored.bin",
        100,
        "recycled",
        Some(false),
    );
    store.add_history_for_scan(&item, "source").unwrap();
    assert!(RecycledTargets::load(&store, &scan("source", 100))
        .unwrap()
        .contains(&item.path));
    assert!(RecycledTargets::load(&store, &scan("older", 99))
        .unwrap()
        .contains(&item.path));
    assert!(
        !RecycledTargets::load(&store, &scan("new-same-second", 100))
            .unwrap()
            .contains(&item.path)
    );
    assert!(!RecycledTargets::load(&store, &scan("new", 101))
        .unwrap()
        .contains(&item.path));
    // The source scan is explicit even if the system clock moved backwards.
    assert!(RecycledTargets::load(&store, &scan("source", 101))
        .unwrap()
        .contains(&item.path));
}

#[test]
fn all_success_history_is_used_independently_of_ui_and_ai_history_limits() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("large-history.sqlite")).unwrap();
    let mut connection = store.connection().unwrap();
    let transaction = connection.transaction().unwrap();
    for index in 0..607 {
        let item = history(
            &format!("h-{index}"),
            &format!(r"D:\Fixture\file-{index}.bin"),
            101 + index,
            "recycled",
            Some(false),
        );
        transaction
            .execute(
                "INSERT INTO history VALUES(?1,?2,?3)",
                rusqlite::params![item.id, item.time, serde_json::to_string(&item).unwrap()],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    let targets = RecycledTargets::load(&store, &scan("old", 100)).unwrap();
    assert_eq!(targets.paths.len(), 607);
    assert!(targets.contains(r"D:\Fixture\file-0.bin"));
    assert!(targets.contains(r"D:\Fixture\file-606.bin"));
    assert_eq!(store.history_page(0, 100).unwrap().items.len(), 100);
    assert_eq!(
        store.recent_recycled_history(0, 1000, 1000).unwrap().len(),
        200
    );
}

#[test]
fn reopen_preserves_scans_and_history_contract_while_filtering_recycled_entries() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("persistent.sqlite")).unwrap();
    let scan = scan("s", 100);
    store.save_scan(&scan).unwrap();
    let file = FileRecord {
        path: r"D:\Fixture\file.bin".into(),
        parent: r"D:\Fixture".into(),
        name: "file.bin".into(),
        logical_bytes: 100,
        complete: true,
        ..Default::default()
    };
    Store::insert_batch(&mut store.connection().unwrap(), &scan.id, &[file]).unwrap();
    let item = history("h", r"D:\Fixture\file.bin", 100, "recycled", Some(false));
    store.add_history_for_scan(&item, &scan.id).unwrap();
    let before_scan = serde_json::to_value(store.scan("s").unwrap()).unwrap();
    let before_file = serde_json::to_value(store.by_path("s", &item.path).unwrap()).unwrap();
    let reopened = Store::open(&store.path).unwrap();
    assert!(RecycledTargets::load(&reopened, &scan)
        .unwrap()
        .contains(&item.path));
    assert_eq!(
        serde_json::to_value(reopened.scan("s").unwrap()).unwrap(),
        before_scan
    );
    assert_eq!(
        serde_json::to_value(reopened.by_path("s", &item.path).unwrap()).unwrap(),
        before_file
    );
    assert_eq!(
        serde_json::to_value(reopened.history_page(0, 20).unwrap().items[0].clone()).unwrap(),
        serde_json::to_value(&item).unwrap()
    );
    assert_eq!(
        reopened
            .connection()
            .unwrap()
            .query_row(
                "SELECT json_extract(data,'$.scanId') FROM history WHERE id='h'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "s"
    );
}

#[test]
fn local_keys_preserve_unc_namespaces_and_match_extended_unc_without_changing_permissions() {
    assert_eq!(
        path_key(r"\\SERVER\Share\\Folder\File.bin"),
        path_key(r"\\?\UNC\server\share\folder\file.bin")
    );
    assert_eq!(
        path_key(r"\\?\UNC\SERVER\\Share\Folder\File.bin"),
        r"\\server\share\folder\file.bin"
    );
    assert_ne!(
        path_key(r"\\server\share\file.bin"),
        path_key(r"server\share\file.bin")
    );
    assert_ne!(
        path_key(r"\\server\share\file.bin"),
        path_key(r"\\server\share-copy\file.bin")
    );
    assert_eq!(
        path_key(r"\\?\D:\Fixture\\File.bin"),
        path_key(r"d:\fixture\file.bin")
    );
}

#[test]
fn history_lookup_uses_time_and_scan_indexes_without_duplicate_branch_results() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("indexed.sqlite")).unwrap();
    for (id, source, time, status) in [
        ("later", "source", 101, "recycled"),
        ("same", "source", 100, "recycled"),
        ("rollback", "source", 99, "recycled"),
        ("other", "other", 100, "recycled"),
        ("failed", "source", 101, "failed"),
    ] {
        store
            .add_history_for_scan(
                &history(
                    id,
                    &format!(r"D:\Fixture\{id}.bin"),
                    time,
                    status,
                    Some(false),
                ),
                source,
            )
            .unwrap();
    }
    store
        .add_history(&history(
            "legacy",
            r"D:\Fixture\legacy.bin",
            101,
            "recycled",
            None,
        ))
        .unwrap();
    let connection = store.connection().unwrap();
    let plan: Vec<String> = connection
        .prepare(&format!("EXPLAIN QUERY PLAN {HISTORY_QUERY}"))
        .unwrap()
        .query_map(rusqlite::params![100, "source"], |row| row.get(3))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(plan
        .iter()
        .any(|step| step.contains("SEARCH history USING INDEX history_time")));
    assert!(plan
        .iter()
        .any(|step| step.contains("SEARCH history USING INDEX history_recycled_scan")));
    assert!(!plan.iter().any(|step| step.starts_with("SCAN history")));
    let paths: Vec<String> = connection
        .prepare(HISTORY_QUERY)
        .unwrap()
        .query_map(rusqlite::params![100, "source"], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(paths.len(), 4);
    assert_eq!(paths.iter().collect::<BTreeSet<_>>().len(), 4);
    let targets = RecycledTargets::load(&store, &scan("source", 100)).unwrap();
    assert_eq!(targets.paths.len(), 4);
    for name in ["later", "same", "rollback", "legacy"] {
        assert!(targets.contains(&format!(r"D:\Fixture\{name}.bin")));
    }
}

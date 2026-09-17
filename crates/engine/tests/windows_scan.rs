use cleaner_domain::*;
use cleaner_engine::{
    cleanup, context,
    scanner::{create_scan, run, ScanJob},
    store::Store,
};
use std::{
    fs,
    sync::{atomic::AtomicBool, Arc},
};
fn scan_fixture(root: &std::path::Path, store: &Store) -> Scan {
    let scan = create_scan(store, root.to_str().unwrap()).unwrap();
    run(
        ScanJob {
            database: store.path.clone(),
            scan_id: scan.id.clone(),
            root: scan.root.clone(),
            settings: Settings::default(),
            journal_probe: None,
        },
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    store.scan(&scan.id).unwrap()
}
#[test]
fn scans_unicode_hardlinks_long_paths_and_allows_content_changes() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("测试目录");
    fs::create_dir(&root).unwrap();
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("index.sqlite")).unwrap();
    fs::write(root.join("普通文件.txt"), "这是 UTF-8 文本").unwrap();
    fs::hard_link(root.join("普通文件.txt"), root.join("第二个硬链接.txt")).unwrap();
    let mut deep = root.clone();
    for _ in 0..15 {
        deep = deep.join("长路径目录_abcdefgh");
    }
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("尾部文件.txt"), "long path").unwrap();
    let s = scan_fixture(&root, &store);
    assert_eq!(s.status, "complete");
    assert_eq!(s.files, 3);
    assert_eq!(s.issues, 0);
    let file = store
        .by_path(&s.id, root.join("普通文件.txt").to_str().unwrap())
        .unwrap();
    let p = cleanup::preview(&store, &s.id, &[file.id]).unwrap();
    assert!(p.items[0].allowed, "{:?}", p.items);
    assert_eq!(p.items[0].bytes, 0);
    fs::write(root.join("普通文件.txt"), "变化后内容").unwrap();
    let changed = cleanup::preview(&store, &s.id, &[file.id]).unwrap();
    assert!(changed.items[0].allowed);
}
#[test]
fn protected_descendants_and_content_consent() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("items");
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("mixed")).unwrap();
    fs::write(root.join("mixed/.env"), "SYNTHETIC=not-a-secret").unwrap();
    fs::write(root.join("note.txt"), "plain safe fixture").unwrap();
    fs::write(root.join("secret.txt"), "password=synthetic-test-value").unwrap();
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("index.sqlite")).unwrap();
    let s = scan_fixture(&root, &store);
    let mixed = store
        .by_path(&s.id, root.join("mixed").to_str().unwrap())
        .unwrap();
    assert!(mixed.has_blocked_children);
    assert!(!cleanup::preview(&store, &s.id, &[mixed.id]).unwrap().items[0].allowed);
    let note = store
        .by_path(&s.id, root.join("note.txt").to_str().unwrap())
        .unwrap();
    assert_eq!(
        context::samples(&store, &s.id, note.id, &[note.id])
            .unwrap()
            .len(),
        1
    );
    let secret = store
        .by_path(&s.id, root.join("secret.txt").to_str().unwrap())
        .unwrap();
    assert!(context::samples(&store, &s.id, secret.id, &[secret.id]).is_err());
    assert!(context::samples(&store, &s.id, note.id, &[secret.id]).is_err());
}
#[test]
fn cancelled_scan_cannot_create_cleanup_preview() {
    let root = tempfile::tempdir().unwrap();
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("index.sqlite")).unwrap();
    let scan = create_scan(&store, root.path().to_str().unwrap()).unwrap();
    run(
        ScanJob {
            database: store.path.clone(),
            scan_id: scan.id.clone(),
            root: scan.root.clone(),
            settings: Settings::default(),
            journal_probe: None,
        },
        Arc::new(AtomicBool::new(true)),
        |_| {},
    )
    .unwrap();
    assert_eq!(store.scan(&scan.id).unwrap().status, "cancelled");
    assert!(cleanup::preview(&store, &scan.id, &[1]).is_err());
}

#[test]
fn incremental_seed_replaces_changed_subtrees_and_preserves_unaffected_data() {
    use cleaner_engine::scanner::SnapshotJournal;
    use cleaner_platform::{filesystem, journal, normalize};
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("delta");
    fs::create_dir_all(root.join("changed")).unwrap();
    fs::create_dir_all(root.join("unchanged")).unwrap();
    fs::write(root.join("changed/old.txt"), "old").unwrap();
    fs::write(root.join("unchanged/keep.txt"), "retained").unwrap();
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("index.sqlite")).unwrap();
    let first = scan_fixture(&root, &store);
    let identity = filesystem::inspect(&root).unwrap().identity;
    let checkpoint = journal::Checkpoint {
        journal_id: 1,
        first_usn: 0,
        next_usn: 10,
        volume: root.to_string_lossy()[..2].into(),
    };
    store
        .put(
            &format!("journal:{}", normalize(root.to_str().unwrap())),
            &SnapshotJournal {
                scan_id: first.id.clone(),
                root_identity: identity,
                checkpoint: checkpoint.clone(),
            },
        )
        .unwrap();
    let changed = filesystem::inspect(&root.join("changed")).unwrap();
    let reference = u128::from_str_radix(
        changed
            .identity
            .as_ref()
            .unwrap()
            .split(':')
            .nth(1)
            .unwrap(),
        16,
    )
    .unwrap();
    fs::rename(
        root.join("changed/old.txt"),
        root.join("changed/renamed.txt"),
    )
    .unwrap();
    fs::write(root.join("changed/new.txt"), "new").unwrap();
    let second = create_scan(&store, root.to_str().unwrap()).unwrap();
    run(
        ScanJob {
            database: store.path.clone(),
            scan_id: second.id.clone(),
            root: second.root.clone(),
            settings: Settings {
                enhanced_scan: true,
                ..Default::default()
            },
            journal_probe: Some(journal::Probe {
                checkpoint,
                changed_parents: Some(vec![reference]),
            }),
        },
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    let result = store.scan(&second.id).unwrap();
    assert_eq!(result.mode, "USN 增量扫描");
    assert_eq!(result.files, 3);
    assert!(store
        .by_path(&second.id, root.join("changed/old.txt").to_str().unwrap())
        .is_err());
    assert!(store
        .by_path(
            &second.id,
            root.join("changed/renamed.txt").to_str().unwrap()
        )
        .is_ok());
    assert!(store
        .by_path(
            &second.id,
            root.join("unchanged/keep.txt").to_str().unwrap()
        )
        .is_ok());

    fs::write(root.join("while-disabled.txt"), "must be found").unwrap();
    let third = create_scan(&store, root.to_str().unwrap()).unwrap();
    let baseline: SnapshotJournal = store
        .get(&format!("journal:{}", normalize(root.to_str().unwrap())))
        .unwrap()
        .unwrap();
    run(
        ScanJob {
            database: store.path.clone(),
            scan_id: third.id.clone(),
            root: third.root,
            settings: Settings::default(),
            journal_probe: Some(journal::Probe {
                checkpoint: baseline.checkpoint,
                changed_parents: Some(vec![]),
            }),
        },
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    let disabled = store.scan(&third.id).unwrap();
    assert_eq!(disabled.mode, "完整扫描");
    assert_eq!(disabled.files, 4);
}

#[test]
fn locked_targets_are_skipped_without_recycling() {
    use std::os::windows::fs::OpenOptionsExt;
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("guard");
    fs::create_dir(&root).unwrap();
    let path = root.join("keep.txt");
    fs::write(&path, "before").unwrap();
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("index.sqlite")).unwrap();
    let scan = scan_fixture(&root, &store);
    let f = store.by_path(&scan.id, path.to_str().unwrap()).unwrap();
    let preview = cleanup::preview(&store, &scan.id, &[f.id]).unwrap();
    assert!(preview.items[0].allowed);
    fs::write(&path, "changed after preview").unwrap();
    let _lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let result =
        cleanup::execute(&store, &preview, true, Arc::new(AtomicBool::new(false))).unwrap();
    assert!(
        matches!(result[0].status.as_str(), "skipped" | "failed"),
        "{:?}",
        result
    );
    assert!(path.exists());
    assert!(!result[0].message.is_empty());
}

#[test]
fn parent_child_targets_are_deduplicated_and_cancelled_batch_is_safe() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("guard");
    fs::create_dir_all(root.join("child")).unwrap();
    let path = root.join("child/file.txt");
    fs::write(&path, "synthetic").unwrap();
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("index.sqlite")).unwrap();
    let scan = scan_fixture(&root, &store);
    let dir = store
        .by_path(&scan.id, root.join("child").to_str().unwrap())
        .unwrap();
    let f = store.by_path(&scan.id, path.to_str().unwrap()).unwrap();
    let preview = cleanup::preview(&store, &scan.id, &[f.id, dir.id]).unwrap();
    if !preview.items[0].allowed {
        let indexed = store.descendants(&scan.id, &dir.path).unwrap();
        let live = cleanup::live_tree(&dir.path).unwrap();
        for f in indexed.iter().chain(&live) {
            println!(
                "REVALIDATION {} {:?} {} {}",
                f.path, f.identity, f.modified_ticks, f.attributes
            );
        }
    }
    assert_eq!(preview.items.len(), 1, "{:?}", preview.items);
    assert!(preview.items[0].allowed, "{:?}", preview.items);
    let result = cleanup::execute(&store, &preview, true, Arc::new(AtomicBool::new(true))).unwrap();
    assert_eq!(result[0].status, "skipped");
    let recorded = result[0].snapshot.as_ref().unwrap();
    assert_eq!(recorded.name, dir.name);
    assert!(recorded.is_dir);
    assert_eq!(
        store.history_page(0, 20).unwrap().items[0]
            .snapshot
            .as_ref()
            .unwrap()
            .name,
        dir.name
    );
    assert!(path.exists());
}

#[test]
#[ignore = "Creates and recycles only a unique test fixture; run explicitly"]
fn recycle_owned_fixture_as_one_shell_batch() {
    let root =
        std::env::temp_dir().join(format!("DiskVista-Recycle-Test-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let paths = [
        root.join("可恢复的测试文件-1.txt"),
        root.join("可恢复的测试文件-2.txt"),
    ];
    for path in &paths {
        fs::write(
            path,
            "DiskVista recycle integration fixture; safe to restore.",
        )
        .unwrap();
    }
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("index.sqlite")).unwrap();
    let scan = scan_fixture(&root, &store);
    let ids: Vec<_> = paths
        .iter()
        .map(|path| store.by_path(&scan.id, path.to_str().unwrap()).unwrap().id)
        .collect();
    let p = cleanup::preview(&store, &scan.id, &ids).unwrap();
    assert_eq!(p.items.len(), paths.len());
    assert!(p.items.iter().all(|item| item.allowed));
    let result = cleanup::execute(&store, &p, true, Arc::new(AtomicBool::new(false))).unwrap();
    println!("TEST ORIGINAL PATHS: {paths:?}");
    println!("RESULT: {}", serde_json::to_string(&result).unwrap());
    assert_eq!(result.len(), paths.len());
    assert!(result.iter().all(|item| item.status == "recycled"));
    assert!(paths.iter().all(|path| !path.exists()));
    // Keep the empty original directory so manual Recycle Bin restoration has a destination.
}

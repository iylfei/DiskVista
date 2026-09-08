use cleaner_domain::{CleanupCheckStage, Settings};
use cleaner_engine::{cleanup, scanner, store::Store};
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[test]
fn preview_reports_work_and_cancels_each_long_stage_without_changing_files() {
    let fixture = tempfile::tempdir().unwrap();
    let target = fixture.path().join("target");
    fs::create_dir(&target).unwrap();
    for index in 0..256 {
        fs::write(target.join(format!("{index}.bin")), b"fixture").unwrap();
    }
    let db = tempfile::tempdir().unwrap();
    let store = Store::open(db.path().join("index.sqlite")).unwrap();
    let scan = scanner::create_scan(&store, fixture.path().to_str().unwrap()).unwrap();
    scanner::run(
        scanner::ScanJob {
            database: store.path.clone(),
            scan_id: scan.id.clone(),
            root: scan.root,
            settings: Settings::default(),
            journal_probe: None,
        },
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap();
    let file = store.by_path(&scan.id, target.to_str().unwrap()).unwrap();
    let ordinary = cleanup::preview(&store, &scan.id, &[file.id]).unwrap();
    assert!(ordinary.items[0].allowed, "{:?}", ordinary.items);
    let mut stages = Vec::new();
    let reported = cleanup::preview_with_progress(
        &store,
        &scan.id,
        &[file.id],
        Arc::new(AtomicBool::new(false)),
        |progress| stages.push(progress),
    )
    .unwrap();
    assert_eq!(reported.items[0].fingerprint, ordinary.items[0].fingerprint);
    assert_eq!(reported.items[0].bytes, ordinary.items[0].bytes);
    let done = stages.last().unwrap();
    assert_eq!(done.stage, CleanupCheckStage::Complete);
    assert_eq!((done.targets_done, done.targets_total), (1, 1));
    for stage in [
        CleanupCheckStage::Snapshot,
        CleanupCheckStage::Filesystem,
        CleanupCheckStage::Protection,
        CleanupCheckStage::Usage,
        CleanupCheckStage::Size,
    ] {
        let cancel = Arc::new(AtomicBool::new(false));
        let result =
            cleanup::preview_with_progress(&store, &scan.id, &[file.id], cancel.clone(), |p| {
                if p.stage == stage && p.checked_entries >= 64 {
                    cancel.store(true, Ordering::Relaxed);
                }
            });
        assert!(
            cancel.load(Ordering::Relaxed),
            "stage {stage:?} was not visited"
        );
        assert!(
            result.unwrap_err().to_string().contains("取消"),
            "stage {stage:?}"
        );
    }
    assert_eq!(fs::read_dir(&target).unwrap().count(), 256);
    assert!(store.history_page(0, 10).unwrap().items.is_empty());
    let result = cleanup::preview_with_progress(
        &store,
        "missing",
        &[file.id],
        Arc::new(AtomicBool::new(true)),
        |_| panic!("pre-cancelled work must not start"),
    );
    assert!(result.unwrap_err().to_string().contains("取消"));
}

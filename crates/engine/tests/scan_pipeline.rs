use cleaner_domain::Settings;
use cleaner_engine::{cleanup, scanner, store::Store};
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[test]
fn directory_workers_drain_all_branches_and_aggregation_can_be_cancelled() {
    let fixture = tempfile::tempdir().unwrap();
    let database = tempfile::tempdir().unwrap();
    for directory in 0..96 {
        let path = fixture.path().join(format!("branch-{directory}"));
        fs::create_dir(&path).unwrap();
        for file in 0..8 {
            fs::write(path.join(format!("file-{file}.bin")), [0u8; 17]).unwrap();
        }
    }
    let store = Store::open(database.path().join("index.sqlite")).unwrap();
    for cancel_aggregation in [false, true] {
        let scan = scanner::create_scan(&store, fixture.path().to_str().unwrap()).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        scanner::run(
            scanner::ScanJob {
                database: store.path.clone(),
                scan_id: scan.id.clone(),
                root: scan.root.clone(),
                settings: Settings::default(),
                journal_probe: None,
            },
            cancel.clone(),
            |progress| {
                if cancel_aggregation && progress.status == "aggregating" {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        )
        .unwrap();
        let finished = store.scan(&scan.id).unwrap();
        let root = store.by_path(&scan.id, &scan.root).unwrap();
        if cancel_aggregation {
            assert_eq!(finished.status, "cancelled");
            assert!(cleanup::preview(&store, &scan.id, &[root.id]).is_err());
        } else {
            assert_eq!(finished.status, "complete");
            assert_eq!(finished.files, 96 * 8);
            assert_eq!(finished.logical_bytes, 96 * 8 * 17);
            assert!(root.complete);
            assert_eq!(finished.issues, 0);
            assert!(store.pending(&scan.id, 1).unwrap().is_empty());
        }
    }
}

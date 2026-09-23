use cleaner_engine::store::Store;
use std::sync::{Arc, Barrier};

#[test]
fn bundled_sqlite_is_patched_and_concurrent_wal_writes_survive_reopen() {
    assert!(rusqlite::version_number() >= 3_051_003);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wal.sqlite");
    let store = Store::open(&path).unwrap();
    let barrier = Arc::new(Barrier::new(5));
    std::thread::scope(|scope| {
        for writer in 0..4 {
            let store = store.clone();
            let barrier = barrier.clone();
            scope.spawn(move || {
                barrier.wait();
                for item in 0..40 {
                    store
                        .put(&format!("writer:{writer}:{item}"), &item)
                        .unwrap();
                }
            });
        }
        let connection = store.connection().unwrap();
        let barrier = barrier.clone();
        scope.spawn(move || {
            barrier.wait();
            for _ in 0..40 {
                connection
                    .execute_batch("PRAGMA wal_checkpoint(PASSIVE)")
                    .unwrap();
                std::thread::yield_now();
            }
        });
    });
    drop(store);
    let reopened = Store::open(path).unwrap();
    for writer in 0..4 {
        for item in 0..40 {
            assert_eq!(
                reopened
                    .get::<u32>(&format!("writer:{writer}:{item}"))
                    .unwrap(),
                Some(item)
            );
        }
    }
    let connection = reopened.connection().unwrap();
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}

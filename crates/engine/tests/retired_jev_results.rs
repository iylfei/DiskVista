use cleaner_engine::store::Store;

#[test]
fn retired_jev_results_remain_stored_but_are_not_active_analysis() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("results.sqlite")).unwrap();
    store
        .connection()
        .unwrap()
        .execute(
            "INSERT INTO analyses(id,scan_id,entry_id,created,data) VALUES(?1,?2,?3,?4,?5)",
            rusqlite::params![
                "old-jev",
                "scan",
                1,
                1,
                r#"{"trace":{"source":"jev"},"status":"success"}"#
            ],
        )
        .unwrap();

    assert!(store.analyses("scan", 1).unwrap().is_empty());
    assert!(store.analyses_for_entries("scan", &[1]).unwrap().is_empty());
    assert!(store.analysis_entry_ids("scan").unwrap().is_empty());
    let count: i64 = store
        .connection()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM analyses WHERE id='old-jev'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

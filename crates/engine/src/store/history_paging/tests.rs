use super::*;

fn fixture(count: usize) -> (tempfile::TempDir, Store, Vec<HistoryItem>) {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("history.sqlite")).unwrap();
    let mut expected: Vec<_> = (0..count)
        .map(|index| HistoryItem {
            id: format!("history-{index:04}"),
            batch_id: format!("batch-{}", index / 7),
            path: format!("D:\\分页夹\\文件-{index}.bin"),
            bytes: index as u64 + 1,
            time: (index / 7) as i64,
            status: ["recycled", "skipped", "failed"][index % 3].into(),
            message: format!("处理记录 {index}"),
            free_space_delta: 0,
            snapshot: None,
        })
        .collect();
    let mut connection = store.connection().unwrap();
    let transaction = connection.transaction().unwrap();
    for item in expected.iter().rev() {
        let mut data = serde_json::to_value(item).unwrap();
        data.as_object_mut().unwrap().remove("snapshot");
        transaction
            .execute(
                "INSERT INTO history VALUES(?1,?2,?3)",
                params![item.id, item.time, data.to_string()],
            )
            .unwrap();
    }
    transaction.commit().unwrap();
    expected.sort_by(|a, b| b.time.cmp(&a.time).then(a.id.cmp(&b.id)));
    (directory, store, expected)
}

#[test]
fn pages_reach_all_history_with_stable_ties_and_preserve_internal_queries() {
    let (_directory, store, expected) = fixture(607);
    let legacy = serde_json::to_value(store.history().unwrap()).unwrap();
    let references =
        serde_json::to_value(store.recent_recycled_history(0, 100, 200).unwrap()).unwrap();
    let mut actual = Vec::new();
    for offset in (0..expected.len()).step_by(20) {
        let page = store.history_page(offset as u64, 20).unwrap();
        assert_eq!(page.total, 607);
        assert_eq!(page.items.len(), (607 - offset).min(20));
        actual.extend(page.items);
    }
    assert_eq!(
        serde_json::to_value(&actual).unwrap(),
        serde_json::to_value(&expected).unwrap()
    );
    assert_eq!(
        serde_json::to_value(store.history_page(580, 20).unwrap().items).unwrap(),
        serde_json::to_value(&expected[580..600]).unwrap()
    );
    let beyond = store.history_page(1_000, 20).unwrap();
    assert_eq!(beyond.total, 607);
    assert!(beyond.items.is_empty());
    assert_eq!(legacy.as_array().unwrap().len(), 500);
    assert_eq!(
        serde_json::to_value(store.history().unwrap()).unwrap(),
        legacy
    );
    assert_eq!(
        serde_json::to_value(store.recent_recycled_history(0, 100, 200).unwrap()).unwrap(),
        references
    );
}

#[test]
fn history_page_bounds_empty_results_and_large_offsets() {
    let (_directory, empty, _) = fixture(0);
    let page = empty.history_page(0, 20).unwrap();
    assert_eq!(page.total, 0);
    assert!(page.items.is_empty());
    let (_directory, store, _) = fixture(120);
    assert_eq!(store.history_page(0, 0).unwrap().items.len(), 1);
    assert_eq!(store.history_page(0, u32::MAX).unwrap().items.len(), 100);
    assert!(store
        .history_page(i64::MAX as u64, 20)
        .unwrap()
        .items
        .is_empty());
    assert!(store.history_page(u64::MAX, 20).is_err());
}

#[test]
fn history_page_only_decodes_requested_rows_and_reads_legacy_snapshots() {
    let (_directory, store, _) = fixture(25);
    store
        .connection()
        .unwrap()
        .execute("INSERT INTO history VALUES('invalid',-1,'not JSON')", [])
        .unwrap();
    let first = store.history_page(0, 20).unwrap();
    assert_eq!(first.total, 26);
    assert_eq!(first.items.len(), 20);
    assert!(first.items.iter().all(|item| item.snapshot.is_none()));
    assert!(store.history_page(25, 20).is_err());
}

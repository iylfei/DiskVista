use super::*;
use cleaner_domain::{Assessment, FileRecord, Scan};
use rusqlite::params;

const ROOT: &str = "D:\\MapQueryFixture";

fn file(parent: &str, name: &str, size: u64, changed: i64) -> FileRecord {
    FileRecord {
        path: format!("{parent}\\{name}"),
        parent: parent.into(),
        name: name.into(),
        logical_bytes: size,
        allocated_bytes: Some(size + 1),
        modified: changed,
        latest_change: changed,
        complete: true,
        enumerated: true,
        file_count: 1,
        assessment: Assessment {
            risk: "review".into(),
            confidence: "low".into(),
            category: "unknown".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn fixture(files: &[FileRecord]) -> (tempfile::TempDir, Store) {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("directory-query.sqlite")).unwrap();
    store
        .save_scan(&Scan {
            id: "s".into(),
            root: ROOT.into(),
            status: "complete".into(),
            ..Default::default()
        })
        .unwrap();
    Store::insert_batch(&mut store.connection().unwrap(), "s", files).unwrap();
    (temp, store)
}

#[test]
fn directory_page_uses_parent_index_despite_many_larger_unrelated_rows() {
    let target = format!("{ROOT}\\Only");
    let mut files: Vec<_> = (0..4_096)
        .map(|n| {
            file(
                &format!("{ROOT}\\Elsewhere"),
                &format!("{n}.bin"),
                1_000_000 + n,
                n as i64,
            )
        })
        .collect();
    files.push(file(&target, "only.bin", 1, 1));
    let (_temp, store) = fixture(&files);
    let connection = store.connection().unwrap();
    for sort in ["size", "name", "activity", "activity_desc", "activity_asc"] {
        for filtered in [false, true] {
            let query = EntryQuery {
                scan_id: "s".into(),
                parent: Some(target.clone()),
                sort: Some(sort.into()),
                minimum_bytes: u64::from(filtered),
                limit: 24,
                ..Default::default()
            };
            let conditions = if filtered {
                "scan_id=? AND parent_key=? AND logical>=1"
            } else {
                "scan_id=? AND parent_key=?"
            };
            let plan = connection
                .prepare(&format!(
                    "EXPLAIN QUERY PLAN {}",
                    page_sql(&query, conditions)
                ))
                .unwrap()
                .query_map(params!["s", normalize(&target), 24, 0], |row| {
                    row.get::<_, String>(3)
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert!(
                plan.iter().any(|line| {
                    line.contains("entries_parent") && line.contains("scan_id=? AND parent_key=?")
                }),
                "{sort}, filtered={filtered}: {plan:?}"
            );
            assert!(!plan.iter().any(|line| line.contains("entries_size")));
            if sort == "size" {
                assert!(
                    !plan.iter().any(|line| line.contains("TEMP B-TREE")),
                    "{plan:?}"
                );
            }
            let count_plan = connection
                .prepare(&format!(
                    "EXPLAIN QUERY PLAN {}",
                    count_sql(&query, conditions)
                ))
                .unwrap()
                .query_map(params!["s", normalize(&target)], |row| {
                    row.get::<_, String>(3)
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert!(
                count_plan.iter().any(|line| {
                    line.contains("entries_parent") && line.contains("scan_id=? AND parent_key=?")
                }),
                "{sort}, filtered={filtered}: {count_plan:?}"
            );
            assert!(!count_plan.iter().any(|line| line.contains("entries_size")));
            let page = store.query(&query).unwrap();
            assert_eq!(page.total, 1);
            assert_eq!(page.items[0].path, format!("{target}\\only.bin"));
        }
    }
    let unscoped = EntryQuery::default();
    assert!(!page_sql(&unscoped, "scan_id=?").contains("INDEXED BY"));
    assert!(!count_sql(&unscoped, "scan_id=?").contains("INDEXED BY"));
}

#[test]
fn directory_sorting_and_ties_stay_stable_across_pages() {
    let mut files: Vec<_> = (0..39)
        .map(|n| file(ROOT, &format!("{:03}.bin", 39 - n), n % 5, (n % 7) as i64))
        .collect();
    files.push(file(
        &format!("{ROOT}\\Nested"),
        "not-a-child.bin",
        1_000,
        1_000,
    ));
    let (_temp, store) = fixture(&files);
    for sort in ["size", "name", "activity", "activity_desc", "activity_asc"] {
        let mut query = EntryQuery {
            scan_id: "s".into(),
            parent: Some("d:/MAPQUERYFIXTURE/".into()),
            sort: Some(sort.into()),
            limit: 7,
            ..Default::default()
        };
        let mut expected: Vec<_> = files[..39]
            .iter()
            .map(|file| store.by_path("s", &file.path).unwrap())
            .collect();
        expected.sort_by(|left, right| {
            let order = match sort {
                "name" => normalize(&left.path).cmp(&normalize(&right.path)),
                "activity" | "activity_desc" => right.latest_change.cmp(&left.latest_change),
                "activity_asc" => left.latest_change.cmp(&right.latest_change),
                _ => right.logical_bytes.cmp(&left.logical_bytes),
            };
            order.then(left.id.cmp(&right.id))
        });
        let mut actual = Vec::new();
        while actual.len() < expected.len() {
            let page = store.query(&query).unwrap();
            assert_eq!(page.total, expected.len() as u64);
            assert!(!page.items.is_empty());
            actual.extend(page.items.iter().map(|file| file.id));
            query.offset += page.items.len() as u32;
        }
        assert_eq!(
            actual,
            expected.iter().map(|file| file.id).collect::<Vec<_>>(),
            "{sort}"
        );
        let beyond = store.query(&query).unwrap();
        assert_eq!(beyond.total, 39);
        assert!(beyond.items.is_empty());
    }
}

#[test]
fn directory_filters_and_empty_parent_keep_exact_counts() {
    let mut selected = file(ROOT, "selected.tmp", 512, 17);
    selected.issue = Some("fixture issue".into());
    selected.assessment.owner = Some("Fixture App".into());
    selected.assessment.category = "temporary".into();
    let mut root = file("", ROOT, 1_000, 19);
    root.path = ROOT.into();
    root.is_dir = true;
    let files = [
        root,
        selected,
        file(ROOT, "small.tmp", 1, 0),
        file(&format!("{ROOT}\\Nested"), "selected.tmp", 999, 20),
    ];
    let (_temp, store) = fixture(&files);
    let query = EntryQuery {
        scan_id: "s".into(),
        parent: Some(ROOT.into()),
        search: Some("SELECTED".into()),
        risk: Some("review".into()),
        owner: Some("Fixture App".into()),
        category: Some("temporary".into()),
        uncertain_only: true,
        issues_only: true,
        minimum_bytes: 100,
        limit: 24,
        ..Default::default()
    };
    let page = store.query(&query).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].path, files[1].path);
    assert_eq!(page.items[0].allocated_bytes, Some(513));
    assert_eq!(page.items[0].latest_change, 17);
    let empty_parent = store
        .query(&EntryQuery {
            scan_id: "s".into(),
            parent: Some(String::new()),
            directories_only: true,
            limit: 24,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(empty_parent.total, 1);
    assert_eq!(empty_parent.items[0].path, ROOT);
    let empty_directory = store
        .query(&EntryQuery {
            scan_id: "s".into(),
            parent: Some(format!("{ROOT}\\Empty")),
            limit: 24,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(empty_directory.total, 0);
    assert!(empty_directory.items.is_empty());
}
#[test]
fn completed_directory_counts_survive_page_and_sort_changes_but_not_filter_changes() {
    let (_temp, store) = fixture(&[file(ROOT, "a.bin", 10, 1), file(ROOT, "b.bin", 20, 2)]);
    let cache = CountCache::default();
    let mut query = EntryQuery {
        scan_id: "s".into(),
        parent: Some(ROOT.into()),
        limit: 1,
        ..Default::default()
    };
    let first = store.query_cached(&query, &cache).unwrap();
    assert_eq!(first.total, 2);
    query.offset = 1;
    query.sort = Some("name".into());
    let second = store.query_cached(&query, &cache).unwrap();
    assert_eq!(second.total, 2);
    assert_eq!(second.items[0].name, "b.bin");
    assert_eq!(cache.stats().sql_counts, 1);
    assert_eq!(cache.stats().hits, 1);
    query.minimum_bytes = 15;
    query.offset = 0;
    assert_eq!(store.query_cached(&query, &cache).unwrap().total, 1);
    assert_eq!(cache.stats().sql_counts, 2);
}

#[test]
fn writes_from_another_connection_and_scan_deletion_invalidate_counts() {
    let (_temp, store) = fixture(&[file(ROOT, "a.bin", 10, 1)]);
    let cache = CountCache::default();
    let query = EntryQuery {
        scan_id: "s".into(),
        parent: Some(ROOT.into()),
        limit: 100,
        ..Default::default()
    };
    assert_eq!(store.query_cached(&query, &cache).unwrap().total, 1);
    Store::insert_batch(
        &mut store.connection().unwrap(),
        "s",
        &[file(ROOT, "b.bin", 20, 2)],
    )
    .unwrap();
    assert_eq!(store.query_cached(&query, &cache).unwrap().total, 2);
    assert_eq!(cache.stats().sql_counts, 2);
    // The write races a read that already obtained its cached total. The response
    // must stay internally consistent, and the next response must see the delete.
    let page = store
        .query_inner(&query, Some(&cache), || {
            store
                .connection()
                .unwrap()
                .execute("DELETE FROM entries WHERE scan_id='s'", [])
                .unwrap();
        })
        .unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.items.len(), 2);
    assert_eq!(store.query_cached(&query, &cache).unwrap().total, 0);
    store
        .connection()
        .unwrap()
        .execute("DELETE FROM scans WHERE id='s'", [])
        .unwrap();
    assert_eq!(store.query_cached(&query, &cache).unwrap().total, 0);
}

#[test]
fn unfinished_scans_are_not_cached_and_caches_do_not_cross_database_paths() {
    let (_temp, store) = fixture(&[file(ROOT, "a.bin", 10, 1)]);
    let (_other_temp, other) = fixture(&[file(ROOT, "a.bin", 10, 1), file(ROOT, "b.bin", 20, 2)]);
    let cache = CountCache::default();
    let query = EntryQuery {
        scan_id: "s".into(),
        parent: Some(ROOT.into()),
        limit: 100,
        ..Default::default()
    };
    assert_eq!(store.query_cached(&query, &cache).unwrap().total, 1);
    assert_eq!(other.query_cached(&query, &cache).unwrap().total, 2);
    let mut scan = store.scan("s").unwrap();
    scan.status = "scanning".into();
    store.save_scan(&scan).unwrap();
    let before = cache.stats().sql_counts;
    for _ in 0..2 {
        assert_eq!(store.query_cached(&query, &cache).unwrap().total, 1);
    }
    assert_eq!(cache.stats().sql_counts - before, 2);
}

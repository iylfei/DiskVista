use super::*;
use cleaner_domain::{Assessment, FileRecord, Scan, Settings};
use cleaner_engine::store::Store;

const ROOT: &str = "D:\\MapFixture";

fn file(path: &str, is_dir: bool, size: u64) -> FileRecord {
    FileRecord {
        path: path.into(),
        parent: path.rsplit_once('\\').unwrap().0.into(),
        name: path.rsplit('\\').next().unwrap().into(),
        is_dir,
        logical_bytes: size,
        allocated_bytes: Some(size + 1),
        modified: 1_710_000_000,
        latest_change: 1_710_000_001,
        accessed: 1_710_000_002,
        created: 1_700_000_000,
        file_count: 1,
        complete: true,
        enumerated: true,
        assessment: Assessment {
            risk: "review".into(),
            category: "unknown".into(),
            confidence: "low".into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn fixture(files: &[FileRecord]) -> (tempfile::TempDir, crate::state::Shared) {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("map.sqlite")).unwrap();
    store
        .save_scan(&Scan {
            id: "s".into(),
            root: ROOT.into(),
            status: "complete".into(),
            started: 1_710_000_000,
            finished: Some(1_710_000_060),
            files: files.len() as u64,
            ..Default::default()
        })
        .unwrap();
    Store::insert_batch(&mut store.connection().unwrap(), "s", files).unwrap();
    (temp, AppState::new(store))
}

fn snapshot(store: &Store) -> (String, Vec<String>) {
    let connection = store.connection().unwrap();
    let mut statement = connection.prepare(
        "SELECT json_array(id,scan_id,path_key,parent_key,is_dir,identity,logical,allocated,file_count,complete,blocked,latest_change,enumerated,issue,risk,owner,rule_id,data,assessment) FROM entries ORDER BY id",
    ).unwrap();
    let entries = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    (
        serde_json::to_string(&store.scan("s").unwrap()).unwrap(),
        entries,
    )
}

#[test]
fn map_retains_current_origins_and_protection_without_rewriting_an_old_scan() {
    let parent = format!("{ROOT}\\Solo");
    let child = format!("{parent}\\OnlyApp");
    let files = [
        file(ROOT, true, 5_000),
        file(&parent, true, 4_000),
        file(&child, true, 3_000),
        file(&format!("{child}\\launcher.exe"), false, 100),
    ];
    let (_temp, state) = fixture(&files);
    // Older scan payloads omit optional timestamps; the indexed values still win.
    state
        .store
        .connection()
        .unwrap()
        .execute(
            "UPDATE entries SET data=json_remove(data,'$.modifiedTicks','$.latestChange')",
            [],
        )
        .unwrap();
    let original = snapshot(&state.store);
    let first = read(&state, "s", &parent).unwrap();
    assert_eq!(first["total"], 1);
    assert_eq!(first["items"][0]["path"], child);
    assert_eq!(first["items"][0]["assessment"]["owner"], "OnlyApp");
    assert_eq!(first["items"][0]["assessment"]["category"], "application");
    assert!(first["items"][0]["assessment"]["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["source"] == "扫描中的可执行文件布局"));
    assert_eq!(first["items"][0]["logicalBytes"], 3_000);
    assert_eq!(first["items"][0]["allocatedBytes"], 3_001);
    assert_eq!(first["items"][0]["latestChange"], 1_710_000_001);
    assert_eq!(first, read(&state, "s", &parent).unwrap());

    let mut settings = Settings::default();
    settings
        .labels
        .insert(child.clone(), "Current label".into());
    settings.protected_paths.push(format!("{child}\\Private"));
    state.store.put("settings", &settings).unwrap();
    state.invalidate_classification();
    let protected = read(&state, "s", &parent).unwrap();
    assert_eq!(
        protected["items"][0]["assessment"]["owner"],
        "Current label"
    );
    assert_eq!(protected["items"][0]["assessment"]["risk"], "protected");
    assert_eq!(protected["parent"]["assessment"]["risk"], "protected");
    assert_eq!(snapshot(&state.store), original);
}

#[test]
fn map_keeps_snapshot_sizes_top_children_total_and_scan_boundaries() {
    let mut files: Vec<_> = (0..40)
        .map(|i| file(&format!("{ROOT}\\{i:02}"), true, i))
        .collect();
    files.push(file(ROOT, true, 9_999));
    let (_temp, state) = fixture(&files);
    Store::insert_batch(
        &mut state.store.connection().unwrap(),
        "other",
        &[file(&format!("{ROOT}\\other-scan-only"), true, 999_999)],
    )
    .unwrap();
    let original = snapshot(&state.store);
    let map = read(&state, "s", "d:/MAPFIXTURE/").unwrap();
    assert_eq!(map["total"], 40);
    let items = map["items"].as_array().unwrap();
    assert_eq!(items.len(), 24);
    assert_eq!(items[0]["logicalBytes"], 39);
    assert_eq!(items[23]["logicalBytes"], 16);
    assert_eq!(map["parent"]["logicalBytes"], 9_999);
    assert_eq!(map["parent"]["assessment"]["risk"], "protected");
    let empty = read(&state, "s", &format!("{ROOT}\\00")).unwrap();
    assert_eq!(empty["total"], 0);
    assert!(empty["items"].as_array().unwrap().is_empty());
    assert!(read(&state, "s", &format!("{ROOT}\\Missing")).is_err());
    assert_eq!(snapshot(&state.store), original);
}

mod benchmark;

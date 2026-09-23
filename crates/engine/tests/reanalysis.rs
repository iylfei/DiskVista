use cleaner_domain::{AnalysisResult, Scan, ANALYSIS_FORMAT_VERSION};
use cleaner_engine::store::Store;

#[test]
fn restart_expires_old_result_and_budget_without_affecting_another_scan() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::open(directory.path().join("reanalysis.sqlite")).unwrap();
    store
        .save_scan(&Scan {
            id: "scan".into(),
            root: r"D:\ScanFixture".into(),
            status: "complete".into(),
            ..Default::default()
        })
        .unwrap();
    let old = AnalysisResult {
        id: "old".into(),
        format_version: ANALYSIS_FORMAT_VERSION,
        scan_id: "scan".into(),
        entry_id: 1,
        fingerprint: "fingerprint".into(),
        config_hash: "config".into(),
        created: 1,
        status: "success".into(),
        message: String::new(),
        assessment: None,
        prompt_tokens: None,
        completion_tokens: None,
        request_id: None,
        request_item_count: 1,
        included_content: false,
        evidence_details: Vec::new(),
        history_references: Vec::new(),
    };
    store.save_analysis(&old).unwrap();
    store.put("llm-budget:scan", &10_u32).unwrap();
    store.put("llm-budget:other", &17_u32).unwrap();

    store.restart_analysis("scan").unwrap();
    assert_eq!(store.get::<u32>("llm-budget:scan").unwrap(), None);
    assert_eq!(store.get::<u32>("llm-budget:other").unwrap(), Some(17));
    assert_eq!(store.analyses("scan", 1).unwrap()[0].status, "stale");

    store.save_analysis(&old).unwrap();
    assert_eq!(store.analyses("scan", 1).unwrap()[0].status, "stale");

    let mut fresh = old.clone();
    fresh.id = "fresh".into();
    fresh.created = 2;
    store.save_analysis(&fresh).unwrap();
    assert_eq!(store.analyses("scan", 1).unwrap()[0].status, "success");
}

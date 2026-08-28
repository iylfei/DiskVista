use super::*;
use cleaner_domain::EntryPage;
use cleaner_engine::{classified_query::Classifier, rules::RuleSet, safety::SafetyPolicy};
use cleaner_platform::normalize;
use serde_json::{json, Value};
use std::{path::Path, time::Instant};

const NOISE_ROWS: u64 = 250_000;
const FIELDS: &str = "id,data,logical,allocated,file_count,complete,blocked,latest_change,enumerated,issue,assessment";

// The pre-optimization query and row overlay, kept only for a reproducible local comparison.
fn legacy_page(store: &Store, scan: &str, parent: &str) -> anyhow::Result<EntryPage> {
    type Row = (
        i64,
        String,
        u64,
        Option<u64>,
        u64,
        bool,
        bool,
        i64,
        bool,
        Option<String>,
        String,
    );
    let connection = store.connection()?;
    let _: String = connection.query_row("SELECT root FROM scans WHERE id=?1", [scan], |row| {
        row.get(0)
    })?;
    let parent = normalize(parent);
    let total = connection.query_row(
        "SELECT count(*) FROM entries WHERE scan_id=?1 AND parent_key=?2",
        (scan, &parent),
        |row| row.get(0),
    )?;
    let mut statement = connection.prepare(&format!(
        "SELECT {FIELDS} FROM entries WHERE scan_id=?1 AND parent_key=?2 ORDER BY logical DESC,id LIMIT 24 OFFSET 0"
    ))?;
    let rows: Vec<Row> = statement
        .query_map((scan, &parent), |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let items = rows
        .into_iter()
        .map(|row| {
            let mut file: FileRecord = serde_json::from_str(&row.1)?;
            file.id = row.0;
            file.logical_bytes = row.2;
            file.allocated_bytes = row.3;
            file.file_count = row.4;
            file.complete = row.5;
            file.has_blocked_children = row.6;
            file.latest_change = row.7;
            file.enumerated = row.8;
            file.issue = row.9;
            file.assessment = serde_json::from_str(&row.10)?;
            Ok(file)
        })
        .collect::<anyhow::Result<_>>()?;
    Ok(EntryPage { items, total })
}

fn legacy_map(state: &AppState, parent: &str) -> Result<Value, String> {
    state.with_classification("s", |classifier, _| {
        let mut root = state.store.by_path("s", parent)?;
        let mut children = legacy_page(&state.store, "s", &root.path)?;
        classifier.apply(&mut root);
        for child in &mut children.items {
            classifier.apply(child);
        }
        Ok(json!({"parent":root,"items":children.items,"total":children.total}))
    })
}

fn milliseconds(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1_000.0
}

fn timings(mut read: impl FnMut() -> Value) -> Value {
    let mut values = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let map = read();
        values.push(milliseconds(start));
        assert_eq!(map["total"], 1);
        assert_eq!(map["items"].as_array().unwrap().len(), 1);
    }
    let mut sorted = values.clone();
    sorted.sort_by(f64::total_cmp);
    json!({"medianMs":sorted[2],"samplesMs":values})
}

fn stages(state: &AppState, parent: &str) -> Value {
    state.invalidate_classification();
    let start = Instant::now();
    let settings = state.store.settings().unwrap();
    let settings_ms = milliseconds(start);
    let start = Instant::now();
    let scan = state.store.scan("s").unwrap();
    let scan_ms = milliseconds(start);
    let start = Instant::now();
    let rules = RuleSet::load(settings.community_enabled).unwrap();
    let rules_ms = milliseconds(start);
    let start = Instant::now();
    let policy = SafetyPolicy::new(settings);
    let policy_ms = milliseconds(start);
    let start = Instant::now();
    let (_, apps) = state.application_index("s", &policy).unwrap();
    let origin_cold_ms = milliseconds(start);
    let start = Instant::now();
    state.application_index("s", &policy).unwrap();
    let origin_warm_ms = milliseconds(start);
    let start = Instant::now();
    let classifier = Classifier::new(&scan, &rules, &policy, &apps);
    let classifier_ms = milliseconds(start);
    let start = Instant::now();
    let mut root = state.store.by_path("s", parent).unwrap();
    let parent_ms = milliseconds(start);
    let start = Instant::now();
    let mut children = state
        .store
        .query(&EntryQuery {
            scan_id: "s".into(),
            parent: Some(parent.into()),
            limit: 24,
            ..Default::default()
        })
        .unwrap();
    let query_ms = milliseconds(start);
    let start = Instant::now();
    classifier.apply(&mut root);
    for child in &mut children.items {
        classifier.apply(child);
    }
    let apply_ms = milliseconds(start);
    json!({"settingsMs":settings_ms,"scanMs":scan_ms,"rulesMs":rules_ms,"policyMs":policy_ms,
        "originColdMs":origin_cold_ms,"originWarmMs":origin_warm_ms,"classifierMs":classifier_ms,
        "parentMs":parent_ms,"queryMs":query_ms,"applyMs":apply_ms})
}

#[test]
#[ignore = "manual synthetic 250k-row performance comparison; no real filesystem scan"]
fn space_map_benchmark() {
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../output/validation");
    std::fs::create_dir_all(&output).unwrap();
    let temp = tempfile::Builder::new()
        .prefix("space-map-")
        .tempdir_in(&output)
        .unwrap();
    let store = Store::open(temp.path().join("fixture.sqlite")).unwrap();
    let parent = format!("{ROOT}\\Solo");
    let child = format!("{parent}\\OnlyApp");
    store
        .save_scan(&Scan {
            id: "s".into(),
            root: ROOT.into(),
            status: "complete".into(),
            files: NOISE_ROWS + 1,
            started: 1_710_000_000,
            finished: Some(1_710_000_001),
            ..Default::default()
        })
        .unwrap();
    Store::insert_batch(
        &mut store.connection().unwrap(),
        "s",
        &[
            file(ROOT, true, 9_000_000),
            file(&parent, true, 100),
            file(&child, true, 99),
            file(&format!("{child}\\launcher.exe"), false, 98),
        ],
    )
    .unwrap();
    {
        let connection = store.connection().unwrap();
        let base = file(&format!("{ROOT}\\Noise\\sample.bin"), false, 1_000_000);
        connection.execute(
            "WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<?1)
             INSERT INTO entries(scan_id,path_key,parent_key,is_dir,logical,allocated,file_count,complete,blocked,latest_change,enumerated,risk,data,assessment)
             SELECT 's',?2||'\\'||i||'.bin',?2,0,1000000+i,1000001+i,1,1,0,1710000001,1,'review',
             json_set(?3,'$.path',?2||'\\'||i||'.bin','$.parent',?2,'$.name',i||'.bin','$.logicalBytes',1000000+i,'$.allocatedBytes',1000001+i),?4 FROM n",
            (NOISE_ROWS, normalize(&base.parent), serde_json::to_string(&base).unwrap(), serde_json::to_string(&base.assessment).unwrap()),
        ).unwrap();
    }
    let state = AppState::new(store);
    let mut reports = Vec::new();
    for community in [false, true] {
        state
            .store
            .put(
                "settings",
                &Settings {
                    community_enabled: community,
                    ..Default::default()
                },
            )
            .unwrap();
        state.invalidate_classification();
        let start = Instant::now();
        let before = legacy_map(&state, &parent).unwrap();
        let old_cold_ms = milliseconds(start);
        let old_warm = timings(|| legacy_map(&state, &parent).unwrap());
        state.invalidate_classification();
        let start = Instant::now();
        let after = read(&state, "s", &parent).unwrap();
        let new_cold_ms = milliseconds(start);
        let new_warm = timings(|| read(&state, "s", &parent).unwrap());
        assert_eq!(before, after);
        reports.push(
            json!({"communityEnabled":community,"oldColdMs":old_cold_ms,"newColdMs":new_cold_ms,
            "oldWarm":old_warm,"newWarm":new_warm,"stages":stages(&state,&parent)}),
        );
    }
    let connection = state.store.connection().unwrap();
    let mut plans = Vec::new();
    for table in ["entries", "entries INDEXED BY entries_parent"] {
        let details = connection.prepare(&format!("EXPLAIN QUERY PLAN SELECT {FIELDS} FROM {table} WHERE scan_id=?1 AND parent_key=?2 ORDER BY logical DESC,id LIMIT 24 OFFSET 0"))
            .unwrap().query_map(("s",normalize(&parent)), |row| row.get::<_,String>(3)).unwrap()
            .collect::<Result<Vec<_>,_>>().unwrap();
        plans.push(json!({"table":table,"details":details}));
    }
    drop(connection);
    let report = json!({"noiseRows":NOISE_ROWS,"directChildren":1,"debugAssertions":cfg!(debug_assertions),
        "coldCache":"application classification cache only; SQLite/OS cache not flushed",
        "plans":plans,"measurements":reports});
    let report = serde_json::to_string_pretty(&report).unwrap();
    println!("{report}");
    std::fs::write(
        output.join("space-map-backend-benchmark.json"),
        report.as_bytes(),
    )
    .unwrap();
    drop(state);
    temp.close().unwrap();
}

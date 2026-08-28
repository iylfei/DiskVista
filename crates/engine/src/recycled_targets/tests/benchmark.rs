use super::*;
use std::time::Instant;

const OLD_QUERY: &str = "
    SELECT json_extract(data,'$.path'), json_extract(data,'$.snapshot.isDir')
    FROM history
    WHERE json_extract(data,'$.status')='recycled'
      AND (time>?1 OR json_extract(data,'$.scanId')=?2)";

fn plan(store: &Store, sql: &str) -> Vec<String> {
    store
        .connection()
        .unwrap()
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap()
        .query_map(rusqlite::params![110, "current"], |row| row.get(3))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
#[ignore = "manual synthetic history lookup comparison; no user database or filesystem scan"]
fn recycled_history_benchmark() {
    let directory = tempfile::tempdir().unwrap();
    let mut cases = Vec::new();
    for count in [1_000, 10_000, 100_000] {
        let store = Store::open(directory.path().join(format!("{count}.sqlite"))).unwrap();
        let mut connection = store.connection().unwrap();
        let transaction = connection.transaction().unwrap();
        let template = serde_json::to_string(&history(
            "template",
            r"D:\Fixture\old.bin",
            100,
            "recycled",
            Some(false),
        ))
        .unwrap();
        transaction
            .execute(
                "WITH RECURSIVE sequence(n) AS (
                    SELECT 1 UNION ALL SELECT n+1 FROM sequence WHERE n<?1
                 )
                 INSERT INTO history(id,time,data)
                 SELECT CAST(n AS TEXT),100,json_set(?2,'$.id',CAST(n AS TEXT),'$.scanId','older')
                 FROM sequence",
                rusqlite::params![count, template],
            )
            .unwrap();
        transaction.commit().unwrap();
        connection
            .execute("DROP INDEX history_recycled_scan", [])
            .unwrap();
        let old_plan = plan(&store, OLD_QUERY);
        let started = Instant::now();
        for _ in 0..5 {
            let connection = store.connection().unwrap();
            let mut statement = connection.prepare(OLD_QUERY).unwrap();
            let rows = statement
                .query_map(rusqlite::params![110, "current"], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert!(rows.is_empty());
        }
        let old_ms = started.elapsed().as_secs_f64() * 200.0;
        let reopened = Store::open(&store.path).unwrap();
        let new_plan = plan(&reopened, HISTORY_QUERY);
        let started = Instant::now();
        for _ in 0..5 {
            assert!(RecycledTargets::load(&reopened, &scan("current", 110))
                .unwrap()
                .paths
                .is_empty());
        }
        let new_ms = started.elapsed().as_secs_f64() * 200.0;
        cases.push(serde_json::json!({
            "historyRows": count,
            "matchingRows": 0,
            "meanMilliseconds": { "before": old_ms, "after": new_ms },
            "beforeQueryPlan": old_plan,
            "afterQueryPlan": new_plan,
        }));
    }
    let report = serde_json::json!({
        "generatedAt": chrono::Utc::now().to_rfc3339(),
        "iterationsPerCase": 5,
        "fixture": "synthetic successful history older than the current scan, from another scan",
        "cases": cases,
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../output/validation/recycled-history-benchmark.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

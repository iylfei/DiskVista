use anyhow::Result;
use cleaner_domain::*;
use cleaner_engine::{
    scanner::{self, ScanJob},
    store::Store,
};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};
fn main() -> Result<()> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!("Provide an empty benchmark output directory"))?,
    );
    if root.exists() {
        anyhow::bail!("Benchmark target must not already exist");
    }
    fs::create_dir_all(&root)?;
    let existing = std::env::args().nth(2).map(PathBuf::from);
    let files = existing.clone().unwrap_or_else(|| root.join("files"));
    let create = Instant::now();
    if existing.is_none() {
        fs::create_dir(&files)?;
        for d in 0..100 {
            let dir = files.join(format!("目录-{d:03}"));
            fs::create_dir(&dir)?;
            for f in 0..1000 {
                fs::write(
                    dir.join(format!("file-{f:04}.txt")),
                    b"Synthetic benchmark data\n",
                )?;
            }
        }
    }
    let creation_ms = create.elapsed().as_millis();
    let store = Store::open(root.join("index/scan.sqlite"))?;
    let s = scanner::create_scan(&store, files.to_str().unwrap())?;
    let start = Instant::now();
    scanner::run(
        ScanJob {
            database: store.path.clone(),
            scan_id: s.id.clone(),
            root: s.root.clone(),
            settings: Settings::default(),
            journal_probe: None,
        },
        Arc::new(AtomicBool::new(false)),
        |_| {},
    )?;
    let scan_ms = start.elapsed().as_millis();
    let scanned = store.scan(&s.id)?;
    anyhow::ensure!(
        scanned.files == 100_000 && scanned.issues == 0,
        "Invalid benchmark scan: {} files, {} issues",
        scanned.files,
        scanned.issues
    );
    let cancelled = scanner::create_scan(&store, files.to_str().unwrap())?;
    let cancel = Arc::new(AtomicBool::new(false));
    let mut requested = None;
    scanner::run(
        ScanJob {
            database: store.path.clone(),
            scan_id: cancelled.id.clone(),
            root: cancelled.root.clone(),
            settings: Settings::default(),
            journal_probe: None,
        },
        cancel.clone(),
        |scan| {
            if scan.files >= 1000 && requested.is_none() {
                requested = Some(Instant::now());
                cancel.store(true, Ordering::SeqCst);
            }
        },
    )?;
    anyhow::ensure!(
        store.scan(&cancelled.id)?.status == "cancelled",
        "Cancellation benchmark failed"
    );
    let cancellation_ms = requested.map(|t| t.elapsed().as_millis());
    let million = Store::open(root.join("million.sqlite"))?;
    let mut c = million.connection()?;
    let insert = Instant::now();
    let mut batch = Vec::new();
    for i in 0..1_000_000 {
        batch.push(FileRecord {
            path: format!("D:\\synthetic\\f-{i:07}.bin"),
            parent: "D:\\synthetic".into(),
            name: format!("f-{i:07}.bin"),
            logical_bytes: (i % 10000 + 1) * 4096,
            allocated_bytes: Some((i % 10000 + 1) * 4096),
            complete: true,
            enumerated: true,
            file_count: 1,
            assessment: Assessment {
                risk: "review".into(),
                ..Default::default()
            },
            ..Default::default()
        });
        if batch.len() == 1000 {
            Store::insert_batch(&mut c, "million", &batch)?;
            batch.clear();
        }
    }
    let insert_ms = insert.elapsed().as_millis();
    let mut queries = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let p = million.query(&EntryQuery {
            scan_id: "million".into(),
            limit: 100,
            ..Default::default()
        })?;
        assert_eq!(p.total, 1_000_000);
        queries.push(t.elapsed().as_micros());
    }
    queries.sort();
    let report = serde_json::json!({"files":scanned.files,"directories":scanned.directories,"issues":scanned.issues,"creationMs":creation_ms,"scanMs":scan_ms,"filesPerSecond":100000000.0/scan_ms as f64,"cancellationMs":cancellation_ms,"millionRowsInsertMs":insert_ms,"queryMedianUs":queries[10],"queryP95Us":queries[19],"note":"Synthetic fixture only; peak process memory measured externally. Files retained at the named output directory."});
    println!("{}", serde_json::to_string_pretty(&report)?);
    fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}

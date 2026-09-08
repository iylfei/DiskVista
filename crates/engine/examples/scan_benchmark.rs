//! Manual benchmark on an explicitly supplied fixture; never recycles files or calls AI.
use anyhow::{Context, Result};
use cleaner_domain::Settings;
use cleaner_engine::{scanner, store::Store};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        args.len() >= 2,
        "usage: scan_benchmark ROOT NEW_DATABASE [cancel_after_ms|aggregate]"
    );
    let root = PathBuf::from(&args[0]);
    let database = PathBuf::from(&args[1]);
    anyhow::ensure!(!database.exists(), "benchmark requires a new database");
    let started = Instant::now();
    let store = Store::open(&database)?;
    let scan = scanner::create_scan(&store, root.to_str().context("Unicode root")?)?;
    let setup_ms = started.elapsed().as_secs_f64() * 1000.0;
    let cancel = Arc::new(AtomicBool::new(false));
    let (finish, finished) = mpsc::channel();
    let cancel_started = Arc::new(std::sync::Mutex::new(None));
    let timer = args
        .get(2)
        .filter(|value| value.as_str() != "aggregate")
        .map(|value| -> Result<_> {
            let milliseconds: u64 = value.parse()?;
            let cancel = cancel.clone();
            let when = cancel_started.clone();
            Ok(std::thread::spawn(move || {
                if finished
                    .recv_timeout(Duration::from_millis(milliseconds))
                    .is_err()
                {
                    *when.lock().unwrap() = Some(Instant::now());
                    cancel.store(true, Ordering::Relaxed);
                }
            }))
        })
        .transpose()?;
    let scanning = Instant::now();
    let mut aggregate_started = None;
    scanner::run(
        scanner::ScanJob {
            database,
            scan_id: scan.id.clone(),
            root: scan.root.clone(),
            settings: Settings::default(),
            journal_probe: None,
        },
        cancel.clone(),
        |progress| {
            if progress.status == "aggregating" && aggregate_started.is_none() {
                aggregate_started = Some(Instant::now());
                if args.get(2).is_some_and(|value| value == "aggregate") {
                    *cancel_started.lock().unwrap() = Some(Instant::now());
                    cancel.store(true, Ordering::Relaxed);
                }
            }
        },
    )?;
    let ended = Instant::now();
    let _ = finish.send(());
    if let Some(timer) = timer {
        timer.join().unwrap();
    }
    let result = store.scan(&scan.id)?;
    let connection = store.connection()?;
    let (rows, metadata_bytes, assessment_bytes): (u64, u64, u64) = connection.query_row(
        "SELECT count(*),COALESCE(sum(length(CAST(data AS BLOB))),0),COALESCE(sum(length(CAST(assessment AS BLOB))),0) FROM entries WHERE scan_id=?1",
        [&scan.id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    // Compare semantic records, independent of concurrent insertion order and row IDs.
    let mut hash = Sha256::new();
    let mut statement = connection.prepare("SELECT path_key,is_dir,identity,logical,allocated,file_count,complete,blocked,latest_change,enumerated,issue,risk,assessment FROM entries WHERE scan_id=?1 ORDER BY path_key")?;
    let entries = statement.query_map([&scan.id], |row| {
        let value = serde_json::json!([
            row.get::<_, String>(0)?,
            row.get::<_, bool>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, u64>(3)?,
            row.get::<_, Option<u64>>(4)?,
            row.get::<_, u64>(5)?,
            row.get::<_, bool>(6)?,
            row.get::<_, bool>(7)?,
            row.get::<_, i64>(8)?,
            row.get::<_, bool>(9)?,
            row.get::<_, Option<String>>(10)?,
            row.get::<_, String>(11)?,
            row.get::<_, String>(12)?
        ]);
        Ok(serde_json::to_vec(&value).unwrap())
    })?;
    for row in entries {
        hash.update(row?);
    }
    drop(statement);
    let before_checkpoint = std::fs::metadata(&store.path)?.len()
        + std::fs::metadata(format!("{}-wal", store.path.display())).map_or(0, |file| file.len());
    let checkpoint = Instant::now();
    connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")?;
    println!(
        "{}",
        serde_json::json!({
            "release": !cfg!(debug_assertions),
            "setupMs": setup_ms,
            "scanMs": ended.duration_since(scanning).as_secs_f64() * 1000.0,
            "enumerationAndSetupMs": aggregate_started.map(|when| when.duration_since(scanning).as_secs_f64() * 1000.0),
            "aggregationAndFinishMs": aggregate_started.map(|when| ended.duration_since(when).as_secs_f64() * 1000.0),
            "cancelLatencyMs": cancel_started.lock().unwrap().map(|when| ended.duration_since(when).as_secs_f64() * 1000.0),
            "status": result.status, "files": result.files, "directories": result.directories,
            "logicalBytes": result.logical_bytes, "issues": result.issues, "rows": rows,
            "metadataBytes": metadata_bytes, "assessmentBytes": assessment_bytes,
            "databaseBytes": std::fs::metadata(&store.path)?.len(),
            "databaseAndWalBeforeCheckpointBytes": before_checkpoint,
            "checkpointMs": checkpoint.elapsed().as_secs_f64() * 1000.0,
            "semanticHash": format!("{:x}", hash.finalize())
        })
    );
    Ok(())
}

//! Manual full scan into an explicitly selected existing app database; no cleanup or AI.
use anyhow::{Context, Result};
use cleaner_engine::{scanner, store::Store};
use std::{
    io::{BufRead, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Instant,
};

fn emit(value: serde_json::Value) {
    println!("{value}");
    let _ = std::io::stdout().flush();
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        args.len() == 2,
        "usage: scan_measurement EXISTING_DATABASE ROOT"
    );
    let database = PathBuf::from(&args[0]);
    anyhow::ensure!(
        database.is_file(),
        "measurement requires an existing app database"
    );
    let _instance = cleaner_platform::process::single_instance()?
        .context("DiskVista is already running; close it before measuring")?;
    let setup = Instant::now();
    let store = Store::open(&database)?;
    let active: u64 = store.connection()?.query_row(
        "SELECT count(*) FROM scans WHERE status IN ('queued','scanning','aggregating')",
        [],
        |row| row.get(0),
    )?;
    anyhow::ensure!(
        active == 0,
        "database contains an active scan; inspect it before measuring"
    );
    let mut settings = store.settings()?;
    // A measurement must enumerate this root instead of silently reusing a USN snapshot.
    // This affects only this job; saved settings are not modified.
    settings.enhanced_scan = false;
    let scan = scanner::create_scan(&store, &args[1])?;
    let setup_ms = setup.elapsed().as_secs_f64() * 1000.0;
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if line.is_err() || line.is_ok_and(|line| line.trim() == "cancel") {
                break;
            }
        }
        flag.store(true, Ordering::Relaxed);
    });
    emit(
        serde_json::json!({"event":"started", "scan":scan, "setupMs":setup_ms,
        "release":!cfg!(debug_assertions), "communityRules":settings.community_enabled,
        "ignoredPathCount":settings.ignored_paths.len()}),
    );
    let started = Instant::now();
    let mut aggregate_started = None;
    let result = scanner::run(
        scanner::ScanJob {
            database,
            scan_id: scan.id.clone(),
            root: scan.root.clone(),
            settings,
            journal_probe: None,
        },
        cancel,
        |progress| {
            if progress.status == "aggregating" && aggregate_started.is_none() {
                aggregate_started = Some(Instant::now());
            }
            emit(
                serde_json::json!({"event":"progress", "elapsedMs":started.elapsed().as_secs_f64()*1000.0, "scan":progress}),
            );
        },
    );
    let ended = Instant::now();
    if let Err(error) = &result {
        let mut failed = store.scan(&scan.id)?;
        failed.status = "failed".into();
        failed.finished = Some(chrono::Utc::now().timestamp());
        failed.message = format!("扫描中止：{error:#}");
        store.save_scan(&failed)?;
    }
    let finished = store.scan(&scan.id)?;
    let root = store.by_path(&scan.id, &scan.root).ok();
    emit(
        serde_json::json!({"event":"finished", "scan":finished, "setupMs":setup_ms,
            "scanMs":ended.duration_since(started).as_secs_f64()*1000.0,
            "enumerationAndSetupMs":aggregate_started.map(|at| at.duration_since(started).as_secs_f64()*1000.0),
            "aggregationAndFinishMs":aggregate_started.map(|at| ended.duration_since(at).as_secs_f64()*1000.0),
            "rootComplete":root.as_ref().map(|file| file.complete),
            "rootProtected":root.as_ref().map(|file| file.assessment.risk == "protected")
        }),
    );
    result
}

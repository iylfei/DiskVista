use cleaner_engine::{
    scanner::{self, ScanJob},
    store::Store,
};
use std::{
    io::{BufRead, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
fn main() {
    if let Err(e) = run() {
        eprintln!("{e:#}");
        std::process::exit(1)
    }
}
fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--journal") {
        return cleaner_platform::elevated::helper(&args);
    }
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if input.len() > 262144 {
        anyhow::bail!("Worker 任务超过长度限制");
    }
    let job: ScanJob = serde_json::from_str(&input)?;
    let database = job.database.clone();
    let scan_id = job.scan_id.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if line.is_err() || line.is_ok_and(|l| l == "cancel") {
                break;
            }
        }
        flag.store(true, Ordering::Relaxed);
    });
    let result = scanner::run(job, cancel, |scan| {
        if let Ok(json) = serde_json::to_string(scan) {
            println!("{json}");
            let _ = std::io::stdout().flush();
        }
    });
    if let Err(e) = &result {
        if let Ok(store) = Store::open(database) {
            if let Ok(mut scan) = store.scan(&scan_id) {
                scan.status = "failed".into();
                scan.message = format!("扫描中止：{e:#}");
                let _ = store.save_scan(&scan);
            }
        }
    }
    result
}

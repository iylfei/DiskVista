use crate::state::{error, Shared, WorkerHandle};
use cleaner_domain::*;
use cleaner_engine::{
    rules::RuleSet,
    safety::SafetyPolicy,
    scanner::{self, ScanJob},
};
use cleaner_platform::{credentials, filesystem, inventory, process::WorkerJob};
use std::{
    io::{BufRead, BufReader, Write},
    os::windows::process::CommandExt,
    process::{Command, Stdio},
    sync::atomic::Ordering,
};
use tauri::State;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    volumes: Vec<Volume>,
    scans: Vec<Scan>,
    settings: Settings,
    has_key: bool,
    access_policy: String,
    analysis_progress: AnalysisProgress,
    scan_locations: Vec<crate::locations::ScanLocation>,
}
#[tauri::command]
pub async fn bootstrap(
    state: State<'_, Shared>,
    app: tauri::AppHandle,
) -> Result<Bootstrap, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        {
            let _guard = state.mutations.lock().unwrap();
            let mut initialized = state.initialized.lock().unwrap();
            if !*initialized {
                state.store.recover_interrupted().map_err(error)?;
                let keep = state.store.settings().map_err(error)?.scan_retention;
                state.store.prune_scans(keep).map_err(error)?;
                *initialized = true;
            }
        }
        let scans = state.store.scans().map_err(error)?;
        Ok(Bootstrap {
            analysis_progress: current_progress(&state, &scans),
            volumes: filesystem::volumes(),
            scans,
            settings: state.store.settings().map_err(error)?,
            has_key: credentials::load().map_err(error)?.is_some(),
            access_policy: inventory::last_access_policy(),
            scan_locations: crate::locations::available(&app),
        })
    })
    .await
}
fn current_progress(state: &Shared, scans: &[Scan]) -> AnalysisProgress {
    let mut progress = state.progress.lock().unwrap().clone();
    if let Some(scan_id) = progress
        .scan_id
        .as_deref()
        .or_else(|| scans.first().map(|scan| scan.id.as_str()))
    {
        if let Some(b) = state.budgets.lock().unwrap().get(scan_id) {
            progress.requests = b.requests.load(Ordering::Relaxed);
            progress.max_requests = b.maximum;
        }
    }
    progress
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    scan: Scan,
    scans: Vec<Scan>,
    analysis_progress: AnalysisProgress,
}
/// Frequent status reads must not enumerate volumes or load credentials/settings.
#[tauri::command]
pub async fn runtime_status(
    state: State<'_, Shared>,
    scan_id: String,
) -> Result<RuntimeStatus, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        let scans = state.store.scans().map_err(error)?;
        let scan = match scans.iter().find(|s| s.id == scan_id) {
            Some(scan) => scan.clone(),
            None => state.store.scan(&scan_id).map_err(error)?,
        };
        Ok(RuntimeStatus {
            scan,
            analysis_progress: current_progress(&state, &scans),
            scans,
        })
    })
    .await
}
#[tauri::command]
pub async fn choose_folder() -> Option<String> {
    rfd::AsyncFileDialog::new()
        .set_title("选择要分析的本地目录")
        .pick_folder()
        .await
        .map(|p| p.path().to_string_lossy().into_owned())
}
fn worker_path() -> Result<std::path::PathBuf, String> {
    let exe = std::env::current_exe().map_err(error)?;
    let sibling = exe.parent().unwrap().join("diskvista-worker.exe");
    if sibling.is_file() {
        return Ok(sibling);
    }
    let dev = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("binaries/diskvista-worker-x86_64-pc-windows-msvc.exe");
    if dev.is_file() {
        Ok(dev)
    } else {
        Err("未找到扫描 Worker，请使用完整安装包或运行构建脚本".into())
    }
}
#[tauri::command]
pub async fn start_scan(state: State<'_, Shared>, root: String) -> Result<Scan, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || start_scan_inner(state, root))
        .await
        .map_err(error)?
}
fn start_scan_inner(state: Shared, root: String) -> Result<Scan, String> {
    let _guard = state.mutations.lock().unwrap();
    if state.cleaning.load(Ordering::SeqCst) {
        return Err("正在回收，暂时不能开始新扫描".into());
    }
    let mut worker = state.worker.lock().unwrap();
    if worker.is_some() {
        return Err("已有扫描正在运行".into());
    }
    let scan = scanner::create_scan(&state.store, &root).map_err(error)?;
    let mut child = Command::new(worker_path()?)
        .creation_flags(0x08000000)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(error)?;
    let job_handle = match WorkerJob::attach(&child) {
        Ok(j) => j,
        Err(e) => {
            let _ = child.kill();
            return Err(error(e));
        }
    };
    let settings = state.store.settings().map_err(error)?;
    let journal_probe = if settings.enhanced_scan {
        let old = state
            .store
            .get::<scanner::SnapshotJournal>(&format!(
                "journal:{}",
                cleaner_platform::normalize(&scan.root)
            ))
            .map_err(error)?;
        let nonce = uuid::Uuid::new_v4().to_string();
        cleaner_platform::elevated::query(
            &worker_path()?,
            scan.root.chars().next().unwrap(),
            &nonce,
            old.as_ref().map(|s| &s.checkpoint),
        )
        .ok()
    } else {
        None
    };
    let job = ScanJob {
        database: state.store.path.clone(),
        scan_id: scan.id.clone(),
        root: scan.root.clone(),
        settings,
        journal_probe,
    };
    let output = child.stdout.take().ok_or("Worker 输出不可用")?;
    writeln!(
        child.stdin.as_mut().ok_or("Worker 输入不可用")?,
        "{}",
        serde_json::to_string(&job).map_err(error)?
    )
    .map_err(error)?;
    *worker = Some(WorkerHandle {
        child,
        _job: job_handle,
    });
    drop(worker);
    let shared = state.clone();
    let scan_id = scan.id.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(output).lines() {
            if line.is_err() {
                break;
            }
        }
        if let Some(mut handle) = shared.worker.lock().unwrap().take() {
            let _ = handle.child.wait();
        }
        if let Ok(s) = shared.store.scan(&scan_id) {
            if s.status == "complete" {
                crate::scan_analysis::auto_analyze(shared.clone(), &scan_id);
            } else if ["scanning", "queued", "aggregating"].contains(&s.status.as_str()) {
                let mut s = s;
                s.status = "interrupted".into();
                s.message = "Worker 意外结束，结果不完整，请重新扫描".into();
                let _ = shared.store.save_scan(&s);
            }
        }
    });
    Ok(scan)
}
#[tauri::command]
pub fn cancel_scan(state: State<'_, Shared>) -> Result<(), String> {
    if let Some(worker) = state.worker.lock().unwrap().as_mut() {
        if let Some(input) = worker.child.stdin.as_mut() {
            writeln!(input, "cancel").map_err(error)?;
        }
    }
    Ok(())
}
#[tauri::command]
pub async fn get_scan(state: State<'_, Shared>, scan_id: String) -> Result<Scan, String> {
    let state = state.inner().clone();
    crate::background::read(move || state.store.scan(&scan_id).map_err(error)).await
}
#[tauri::command]
pub async fn query_entries(
    state: State<'_, Shared>,
    query: EntryQuery,
) -> Result<EntryPage, String> {
    let state = state.inner().clone();
    crate::background::read(move || state.query_entries(&query)).await
}
#[tauri::command]
pub async fn application_units(
    state: State<'_, Shared>,
    scan_id: String,
    search: String,
    offset: u32,
    limit: u32,
) -> Result<ApplicationUnitPage, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.store.require_finished(&scan_id).map_err(error)?;
        let policy = SafetyPolicy::new(state.store.settings().map_err(error)?);
        let (key, index) = state.application_index(&scan_id, &policy)?;
        let mut snapshot = state.units_snapshot.lock().unwrap();
        if snapshot.as_ref().is_none_or(|(old, _)| old != &key) {
            let units =
                cleaner_engine::units::build_indexed(&state.store, &scan_id, &policy, &index)
                    .map_err(error)?;
            *snapshot = Some((key, units));
        }
        let units = &snapshot.as_ref().unwrap().1;
        Ok(cleaner_engine::units::page(
            units,
            &search,
            offset as usize,
            limit as usize,
        ))
    })
    .await
    .map_err(error)?
}
#[tauri::command]
pub async fn space_map(
    state: State<'_, Shared>,
    scan_id: String,
    parent: String,
) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        state.with_classification(&scan_id, |classifier, _| {
            let mut root = state.store.by_path(&scan_id, &parent)?;
            let mut children = state.store.query(&EntryQuery {
                scan_id: scan_id.clone(),
                parent: Some(root.path.clone()),
                limit: 24,
                ..Default::default()
            })?;
            classifier.apply(&mut root);
            for file in &mut children.items {
                classifier.apply(file);
            }
            Ok(serde_json::json!({"parent":root,"items":children.items,"total":children.total}))
        })
    })
    .await
}
#[tauri::command]
pub async fn entry_detail(
    state: State<'_, Shared>,
    scan_id: String,
    entry_id: i64,
) -> Result<FileRecord, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        let mut f = state.classified_entry(&scan_id, entry_id)?;
        let scan = state.store.scan(&scan_id).map_err(error)?;
        if let Ok(evidence) = cleaner_platform::metadata::executable_evidence(&f.path) {
            f.assessment.evidence.extend(evidence);
        }
        if let Some(previous) = state.store.scans().map_err(error)?.iter().find(|s| {
            s.id != scan_id
                && s.status == "complete"
                && cleaner_platform::normalize(&s.root) == cleaner_platform::normalize(&scan.root)
        }) {
            if let Ok(old) = state.store.by_path(&previous.id, &f.path) {
                f.assessment.evidence.push(Evidence {
                    source: "上次扫描记录".into(),
                    detail: format!(
                        "上次大小 {} 字节，本次 {} 字节；{}。快照变化不是应用实际使用证明。",
                        old.logical_bytes,
                        f.logical_bytes,
                        if old.latest_change == f.latest_change
                            && old.logical_bytes == f.logical_bytes
                        {
                            "未观察到大小或时间变化"
                        } else {
                            "观察到大小或时间变化"
                        }
                    ),
                });
            }
        }
        Ok(f)
    })
    .await
}
#[tauri::command]
pub async fn groups(
    state: State<'_, Shared>,
    scan_id: String,
    kind: String,
) -> Result<Vec<Group>, String> {
    let state = state.inner().clone();
    crate::background::read(move || state.classified_groups(&scan_id, &kind)).await
}
#[tauri::command]
pub async fn rules(state: State<'_, Shared>) -> Result<serde_json::Value, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        let r = RuleSet::load(state.store.settings().map_err(error)?.community_enabled)
            .map_err(error)?;
        let mut community: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/rules/community.json"))
                .map_err(error)?;
        if let Some(skipped) = community["skipped"].as_array() {
            let count = skipped.len();
            community["skippedCount"] = count.into();
        }
        community.as_object_mut().unwrap().remove("skipped");
        community.as_object_mut().unwrap().remove("rules");
        Ok(serde_json::json!({"version":r.version,"rules":r.rules,"community":community}))
    })
    .await
}

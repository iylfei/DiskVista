use crate::{
    ai::{self, AnalysisOutcome},
    state::{error, Shared},
};
use cleaner_domain::{AnalysisProgress, FileRecord, LlmSettings, Scan, Settings};
use cleaner_engine::{
    application_index::ApplicationIndex, classified_query::Classifier, rules::RuleSet,
    safety::SafetyPolicy,
};
use cleaner_llm::client::{self, Budget};
use std::{
    collections::{HashSet, VecDeque},
    sync::{atomic::Ordering, Mutex},
};
use tauri::State;
pub(crate) mod grouping;

const MINIMUM_BATCH_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq)]
enum Trigger {
    Manual,
    Automatic,
}

fn validate(settings: &LlmSettings, status: &str, trigger: Trigger) -> Result<(), String> {
    if !settings.enabled {
        return Err("请先在设置中启用 AI".into());
    }
    if settings.base_url.trim().is_empty() || settings.model.trim().is_empty() {
        return Err("请先在设置中填写 AI 服务地址和模型".into());
    }
    client::endpoint(&settings.base_url).map_err(error)?;
    if status != "complete" {
        return Err("请先完成扫描，再分析扫描结果".into());
    }
    if trigger == Trigger::Automatic && (!settings.automatic || !settings.metadata_consent) {
        return Err("自动分析尚未启用或授权".into());
    }
    Ok(())
}

struct AnalysisLease(Shared);

impl Drop for AnalysisLease {
    fn drop(&mut self) {
        if let Ok(mut progress) = self.0.progress.lock() {
            progress.active = false;
        }
        self.0.analysis_busy.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
pub async fn analyze_scan(
    state: State<'_, Shared>,
    scan_id: String,
) -> Result<AnalysisProgress, String> {
    let state = state.inner().clone();
    crate::background::read(move || start(state, &scan_id, Trigger::Manual)).await
}

pub fn auto_analyze(state: Shared, scan_id: &str) {
    let Ok(settings) = state.store.settings() else {
        return;
    };
    if settings.llm.enabled && settings.llm.automatic && settings.llm.metadata_consent {
        let _ = start(state, scan_id, Trigger::Automatic);
    }
}

fn start(state: Shared, scan_id: &str, trigger: Trigger) -> Result<AnalysisProgress, String> {
    let setup_guard = state.mutations.lock().unwrap();
    let settings = state.store.settings().map_err(error)?;
    let scan = state.store.scan(scan_id).map_err(error)?;
    validate(&settings.llm, &scan.status, trigger)?;
    if state.cleaning.load(Ordering::SeqCst) {
        return Err("正在回收文件，请稍后分析".into());
    }
    if state.analysis_busy.swap(true, Ordering::SeqCst) {
        return Err("已有 AI 分析任务运行，请等待或取消".into());
    }
    let lease = AnalysisLease(state.clone());
    let budget = ai::budget(&state, scan_id, settings.llm.max_requests);
    budget.cancel.store(false, Ordering::SeqCst);
    drop(setup_guard);
    let mut candidates = collect_candidates(&state, &scan, &settings)?;
    let queued = candidates.len() as u32;
    let ids: Vec<_> = candidates.iter().map(|file| file.id).collect();
    let reusable = crate::analysis_results::reusable_ids(&state, scan_id, &ids)?;
    candidates.retain(|file| !reusable.contains(&file.id));
    let cached = queued - candidates.len() as u32;
    if !candidates.is_empty() && budget.requests.load(Ordering::Relaxed) >= budget.maximum {
        return Err("本次扫描的 AI 请求上限已用完，可在设置中调整上限后继续。".into());
    }
    let progress = AnalysisProgress {
        scan_id: Some(scan_id.into()),
        active: !candidates.is_empty(),
        queued,
        finished: cached,
        requests: budget.requests.load(Ordering::Relaxed),
        max_requests: budget.maximum,
        message: if candidates.is_empty() && cached > 0 {
            format!("已复用 {cached} 项有效分析，没有需要重复请求的文件。")
        } else if candidates.is_empty() {
            "没有来源不明确且超过当前大小门槛的待分析文件。".into()
        } else {
            "正在按目录组织批量分析…".into()
        },
    };
    *state.progress.lock().unwrap() = progress.clone();
    if candidates.is_empty() {
        return Ok(progress);
    }
    let scan_id = scan_id.to_owned();
    std::thread::Builder::new()
        .name("scan-ai-analysis".into())
        .spawn(move || {
            let _lease = lease;
            run_batch(
                &state,
                &scan_id,
                &settings.llm,
                trigger,
                budget,
                candidates,
                cached,
            );
        })
        .map_err(|_| "无法启动 AI 分析任务，请稍后重试".to_owned())?;
    Ok(progress)
}

fn collect_candidates(
    state: &Shared,
    scan: &Scan,
    settings: &Settings,
) -> Result<VecDeque<FileRecord>, String> {
    let scan_id = scan.id.as_str();
    let minimum_bytes = settings.llm.minimum_bytes.max(MINIMUM_BATCH_BYTES);
    let mut candidates = state
        .store
        .analysis_candidate_pool(scan_id, minimum_bytes)
        .map_err(error)?;
    let policy = SafetyPolicy::new(settings.clone());
    let (_, apps) = state.application_index(scan_id, &policy)?;
    let rules = RuleSet::load(settings.community_enabled).map_err(error)?;
    let classifier = Classifier::new(scan, &rules, &policy, &apps);
    for file in &mut candidates {
        classifier.apply(file);
    }
    Ok(select_candidates(candidates, &policy, &apps, minimum_bytes))
}

fn select_candidates(
    files: Vec<FileRecord>,
    policy: &SafetyPolicy,
    apps: &ApplicationIndex,
    minimum_bytes: u64,
) -> VecDeque<FileRecord> {
    let mut seen = HashSet::new();
    let mut ranked: Vec<_> = files
        .into_iter()
        .filter(|file| seen.insert(file.id) && eligible(file, policy, apps, minimum_bytes))
        .collect();
    ranked.sort_by(|a, b| b.logical_bytes.cmp(&a.logical_bytes).then(a.id.cmp(&b.id)));
    ranked.into()
}

pub(crate) fn eligible(
    file: &FileRecord,
    policy: &SafetyPolicy,
    apps: &ApplicationIndex,
    minimum_bytes: u64,
) -> bool {
    !file.is_dir
        && file.logical_bytes > minimum_bytes.max(MINIMUM_BATCH_BYTES)
        && file.assessment.owner.is_none()
        && file.assessment.rule_id.is_none()
        && file.complete
        && !file.has_blocked_children
        && !matches!(file.assessment.risk.as_str(), "protected" | "keep")
        && file.assessment.protected_reason.is_none()
        && policy.reason(file).is_none()
        && apps.installed_reason(file).is_none()
        && !policy
            .settings
            .excluded_llm_paths
            .iter()
            .any(|path| cleaner_platform::within(&file.path, path))
}

#[derive(Default)]
struct Outcomes {
    completed: u32,
    cached: u32,
    skipped: u32,
    failed: u32,
    last_error: Option<String>,
    quota_skipped: bool,
}

fn run_batch(
    state: &Shared,
    scan_id: &str,
    settings: &LlmSettings,
    trigger: Trigger,
    budget: Budget,
    candidates: VecDeque<FileRecord>,
    cached: u32,
) {
    let queue = Mutex::new(candidates);
    let outcomes = Mutex::new(Outcomes {
        cached,
        ..Default::default()
    });
    std::thread::scope(|scope| {
        for _ in 0..settings.concurrency.clamp(1, 2) {
            let budget = &budget;
            let queue = &queue;
            let outcomes = &outcomes;
            scope.spawn(move || loop {
                if budget.cancel.load(Ordering::Relaxed)
                    || budget.requests.load(Ordering::Relaxed) >= budget.maximum
                {
                    break;
                }
                let current = match state.store.settings() {
                    Ok(current)
                        if current.llm.enabled
                            && (trigger == Trigger::Manual
                                || (current.llm.automatic && current.llm.metadata_consent)) =>
                    {
                        current
                    }
                    _ => {
                        budget.cancel.store(true, Ordering::SeqCst);
                        break;
                    }
                };
                let files = grouping::take(
                    &mut queue.lock().unwrap(),
                    grouping::item_limit(&current.llm),
                );
                if files.is_empty() {
                    break;
                }
                state.progress.lock().unwrap().message =
                    format!("正在准备 {} 个文件的批量分析…", files.len());
                let mut contexts = Vec::new();
                for file in files {
                    match ai::snapshot_context(state, scan_id, file.id) {
                        Ok(context) => contexts.push(context),
                        Err(message) => {
                            let mut outcomes = outcomes.lock().unwrap();
                            outcomes.skipped += 1;
                            outcomes.last_error = Some(message);
                            state.progress.lock().unwrap().finished += 1;
                        }
                    }
                }
                for batch in grouping::pack(contexts) {
                    let batch = match batch {
                        Ok(batch) => batch,
                        Err(message) => {
                            let mut outcomes = outcomes.lock().unwrap();
                            outcomes.skipped += 1;
                            outcomes.last_error = Some(message);
                            state.progress.lock().unwrap().finished += 1;
                            continue;
                        }
                    };
                    let count = batch.len() as u32;
                    let result =
                        crate::ai_batch::run(state, batch, budget, trigger == Trigger::Automatic);
                    let mut outcomes = outcomes.lock().unwrap();
                    match result {
                        Ok(results) => {
                            for result in results {
                                match result {
                                    Ok(AnalysisOutcome::Completed) => outcomes.completed += 1,
                                    Ok(AnalysisOutcome::Cached) => outcomes.cached += 1,
                                    Ok(AnalysisOutcome::Failed(message)) => {
                                        outcomes.failed += 1;
                                        outcomes.last_error = Some(message);
                                    }
                                    Err(message) => {
                                        outcomes.skipped += 1;
                                        outcomes.last_error = Some(message);
                                    }
                                }
                            }
                        }
                        Err(message) => {
                            outcomes.quota_skipped |=
                                budget.requests.load(Ordering::Relaxed) >= budget.maximum;
                            outcomes.skipped += count;
                            outcomes.last_error = Some(message);
                            state.progress.lock().unwrap().finished += count;
                        }
                    }
                }
            });
        }
    });
    let outcomes = outcomes.into_inner().unwrap();
    let state_text = if budget.cancel.load(Ordering::Relaxed) {
        "AI 分析已取消"
    } else if outcomes.quota_skipped || !queue.into_inner().unwrap().is_empty() {
        "本次扫描的 AI 请求上限已用完"
    } else {
        "AI 分析完成"
    };
    let mut progress = state.progress.lock().unwrap();
    progress.requests = budget.requests.load(Ordering::Relaxed);
    progress.message = format!(
        "{state_text}：新分析 {} 项，复用 {} 项，跳过 {} 项，失败 {} 项。",
        outcomes.completed, outcomes.cached, outcomes.skipped, outcomes.failed
    );
    if let Some(message) = outcomes.last_error {
        progress.message.push_str(&format!("最近问题：{message}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_domain::{Assessment, HistoryItem, Settings};
    use cleaner_engine::store::Store;

    fn settings() -> LlmSettings {
        LlmSettings {
            enabled: true,
            base_url: "https://example.com/v1".into(),
            model: "test".into(),
            ..Default::default()
        }
    }
    fn candidate(id: i64, path: &str, known: bool) -> FileRecord {
        FileRecord {
            id,
            path: path.into(),
            name: path.rsplit('\\').next().unwrap().into(),
            complete: true,
            logical_bytes: 200 * 1024 * 1024,
            assessment: Assessment {
                risk: "review".into(),
                confidence: if known { "medium" } else { "low" }.into(),
                rule_id: known.then(|| "known-example".into()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn candidate_collection_uses_current_rules_and_protection_without_external_ai() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("candidates.sqlite")).unwrap();
        let scan = Scan {
            id: "s".into(),
            root: "D:\\CandidateFixture".into(),
            status: "complete".into(),
            ..Default::default()
        };
        store.save_scan(&scan).unwrap();
        let mut old = candidate(0, "D:\\CandidateFixture\\old-rule.bin", true);
        old.assessment.risk = "protected".into();
        old.assessment.protected_reason = Some("撤销前的保护规则".into());
        let protected = candidate(0, "D:\\CandidateFixture\\protected.bin", false);
        let excluded = candidate(0, "D:\\CandidateFixture\\excluded.bin", false);
        let mut small = candidate(0, "D:\\CandidateFixture\\small.bin", false);
        small.logical_bytes = 99;
        Store::insert_batch(
            &mut store.connection().unwrap(),
            "s",
            &[old, protected, excluded, small],
        )
        .unwrap();
        let mut settings = Settings::default();
        settings.llm.minimum_bytes = 100;
        settings
            .protected_paths
            .push("D:\\CandidateFixture\\protected.bin".into());
        settings
            .excluded_llm_paths
            .push("D:\\CandidateFixture\\excluded.bin".into());
        store.put("settings", &settings).unwrap();
        let state = crate::state::AppState::new(store);
        let candidates = collect_candidates(&state, &scan, &settings).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "old-rule.bin");
        assert_eq!(candidates[0].assessment.risk, "review");
        assert!(candidates[0].assessment.rule_id.is_none());
        let saved = state.store.by_path("s", &candidates[0].path).unwrap();
        assert_eq!(saved.assessment.risk, "protected");
        assert!(saved.assessment.rule_id.is_some());
        let pool = state.store.analysis_candidate_pool("s", 100).unwrap();
        assert!(pool.iter().any(|file| file.path == candidates[0].path));
        assert_eq!(state.progress.lock().unwrap().requests, 0);
    }
    #[test]
    fn only_unknown_source_large_files_are_selected_even_when_history_is_enabled() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("index.sqlite")).unwrap();
        let mut settings = Settings::default();
        settings.llm.history_reference_enabled = true;
        store.put("settings", &settings).unwrap();
        store
            .add_history(&HistoryItem {
                id: "h".into(),
                batch_id: "b".into(),
                path: "D:\\Example\\editorbundle-1.zip".into(),
                bytes: 100,
                time: chrono::Utc::now().timestamp(),
                status: "recycled".into(),
                message: String::new(),
                free_space_delta: 0,
                snapshot: None,
            })
            .unwrap();
        let policy = SafetyPolicy::new(settings);
        let apps = ApplicationIndex::new(&[], &policy);
        let known = candidate(1, "D:\\Example\\editorbundle-2.zip", true);
        let unrelated = candidate(2, "D:\\Example\\holiday.zip", true);
        let mut ancestor = candidate(3, "D:\\Example", false);
        ancestor.is_dir = true;
        ancestor.logical_bytes = 10_000 * 1024 * 1024;
        let unknown = candidate(4, "D:\\Other\\unknown.bin", false);
        let mut small = candidate(5, "D:\\Example\\editorbundle-3.zip", true);
        small.logical_bytes = 99;
        let mut protected = candidate(6, "D:\\Example\\editorbundle-4.zip", true);
        protected.assessment.risk = "protected".into();
        let mut known_owner = candidate(7, "D:\\Example\\owned.bin", false);
        known_owner.assessment.owner = Some("已知应用".into());
        let mut larger = candidate(8, "D:\\Other\\larger.bin", false);
        larger.logical_bytes = 500 * 1024 * 1024;
        let mut boundary = candidate(9, "D:\\Other\\boundary.bin", false);
        boundary.logical_bytes = MINIMUM_BATCH_BYTES;
        let selected = select_candidates(
            vec![
                ancestor,
                unrelated,
                small,
                protected,
                known,
                unknown,
                known_owner,
                larger,
                boundary,
            ],
            &policy,
            &apps,
            100,
        );
        assert_eq!(
            selected.iter().map(|f| f.id).collect::<Vec<_>>(),
            vec![8, 4]
        );
        let selected = select_candidates(
            (1..=150)
                .map(|id| candidate(id, &format!("D:\\Other\\unknown-{id}.bin"), false))
                .collect(),
            &policy,
            &apps,
            100,
        );
        assert_eq!(selected.len(), 150);
    }

    #[test]
    fn explicit_scan_analysis_does_not_require_background_automation() {
        let mut settings = settings();
        assert!(validate(&settings, "complete", Trigger::Manual).is_ok());
        assert!(validate(&settings, "complete", Trigger::Automatic).is_err());
        settings.automatic = true;
        assert!(validate(&settings, "complete", Trigger::Automatic).is_err());
        settings.metadata_consent = true;
        assert!(validate(&settings, "complete", Trigger::Automatic).is_ok());
    }

    #[test]
    fn requires_enabled_configured_ai_and_a_completed_scan() {
        let mut settings = settings();
        settings.enabled = false;
        assert!(validate(&settings, "complete", Trigger::Manual).is_err());
        settings.enabled = true;
        settings.model.clear();
        assert!(validate(&settings, "complete", Trigger::Manual).is_err());
        settings.model = "test".into();
        for status in ["queued", "scanning", "aggregating", "cancelled", "failed"] {
            assert!(validate(&settings, status, Trigger::Manual).is_err());
        }
    }

    #[test]
    fn start_reads_limits_after_pending_settings_mutation_without_overwriting_cancellation() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("startup.sqlite")).unwrap();
        store
            .save_scan(&Scan {
                id: "s".into(),
                root: "D:\\StartupFixture".into(),
                status: "complete".into(),
                ..Default::default()
            })
            .unwrap();
        let mut initial = Settings {
            llm: settings(),
            ..Default::default()
        };
        initial.llm.max_requests = 100;
        store.put("settings", &initial).unwrap();
        let state = crate::state::AppState::new(store);
        let guard = state.mutations.lock().unwrap();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let running = state.clone();
        let thread = std::thread::spawn(move || {
            entered_tx.send(()).unwrap();
            done_tx.send(start(running, "s", Trigger::Manual)).unwrap();
        });
        entered_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        assert!(matches!(
            done_rx.recv_timeout(std::time::Duration::from_millis(50)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        initial.llm.max_requests = 1;
        state.store.put("settings", &initial).unwrap();
        drop(guard);
        let progress = done_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap();
        thread.join().unwrap();
        assert_eq!(progress.max_requests, 1);
        assert_eq!(progress.requests, 0);
        assert!(!progress.active);
    }
}

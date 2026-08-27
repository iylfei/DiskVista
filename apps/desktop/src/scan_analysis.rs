use crate::{
    ai::{self, AnalysisOutcome},
    state::{error, Shared},
};
use cleaner_domain::{AnalysisProgress, EntryQuery, FileRecord, LlmSettings};
use cleaner_llm::client::{self, Budget};
use std::{
    collections::VecDeque,
    sync::{atomic::Ordering, Mutex},
};
use tauri::State;

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
    if budget.requests.load(Ordering::Relaxed) >= budget.maximum {
        return Err("本次扫描的 AI 请求上限已用完，可在设置中调整上限后继续。".into());
    }
    budget.cancel.store(false, Ordering::SeqCst);
    let candidates = state
        .store
        .query(&EntryQuery {
            scan_id: scan_id.into(),
            risk: Some("review".into()),
            minimum_bytes: settings.llm.minimum_bytes,
            uncertain_only: true,
            limit: 100,
            ..Default::default()
        })
        .map_err(error)?
        .items
        .into_iter()
        .filter(|file| {
            file.assessment.rule_id.is_none()
                && file.assessment.confidence == "low"
                && file.complete
                && !file.has_blocked_children
                && !settings
                    .excluded_llm_paths
                    .iter()
                    .any(|path| cleaner_platform::within(&file.path, path))
        })
        .collect::<VecDeque<_>>();
    let progress = AnalysisProgress {
        scan_id: Some(scan_id.into()),
        active: !candidates.is_empty(),
        queued: candidates.len() as u32,
        finished: 0,
        requests: budget.requests.load(Ordering::Relaxed),
        max_requests: budget.maximum,
        message: if candidates.is_empty() {
            "没有符合当前门槛的待分析项目。".into()
        } else {
            "正在分析扫描结果…".into()
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
            run_batch(&state, &scan_id, &settings.llm, trigger, budget, candidates);
        })
        .map_err(|_| "无法启动 AI 分析任务，请稍后重试".to_owned())?;
    Ok(progress)
}

#[derive(Default)]
struct Outcomes {
    completed: u32,
    cached: u32,
    skipped: u32,
    failed: u32,
    last_error: Option<String>,
}

fn run_batch(
    state: &Shared,
    scan_id: &str,
    settings: &LlmSettings,
    trigger: Trigger,
    budget: Budget,
    candidates: VecDeque<FileRecord>,
) {
    let queue = Mutex::new(candidates);
    let outcomes = Mutex::new(Outcomes::default());
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
                let authorized = state.store.settings().is_ok_and(|current| {
                    current.llm.enabled
                        && (trigger == Trigger::Manual
                            || (current.llm.automatic && current.llm.metadata_consent))
                });
                if !authorized {
                    budget.cancel.store(true, Ordering::SeqCst);
                    break;
                }
                let Some(file) = queue.lock().unwrap().pop_front() else {
                    break;
                };
                let result = ai::checked_context(state, scan_id, file.id)
                    .and_then(|context| ai::run_one(state, context, vec![], budget));
                let mut outcomes = outcomes.lock().unwrap();
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
                        state.progress.lock().unwrap().finished += 1;
                    }
                }
            });
        }
    });
    let outcomes = outcomes.into_inner().unwrap();
    let state_text = if budget.cancel.load(Ordering::Relaxed) {
        "AI 分析已取消"
    } else if !queue.into_inner().unwrap().is_empty() {
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

    fn settings() -> LlmSettings {
        LlmSettings {
            enabled: true,
            base_url: "https://example.com/v1".into(),
            model: "test".into(),
            ..Default::default()
        }
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
}

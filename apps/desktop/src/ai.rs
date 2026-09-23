use crate::state::{error, AppState, ContextPreview, SamplePreview, Shared};
use cleaner_domain::*;
use cleaner_engine::{
    context::{self, Sample},
    rules::RuleSet,
};
use cleaner_llm::client::{self, Budget};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::{atomic::Ordering, Arc};
use tauri::State;

pub(crate) fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
pub(crate) fn budget(state: &Shared, scan: &str, max: u32) -> Budget {
    let mut budgets = state.budgets.lock().unwrap();
    let entry = budgets.entry(scan.into()).or_insert_with(|| {
        let key = format!("llm-budget:{scan}");
        let mut b = Budget::new(max);
        b.requests.store(
            state.store.get::<u32>(&key).ok().flatten().unwrap_or(0),
            Ordering::SeqCst,
        );
        let store = state.store.clone();
        b.persist = Some(Arc::new(move |n| store.put_counter_max(&key, n)));
        b
    });
    entry.maximum = max;
    entry.clone()
}
pub(crate) fn snapshot_context(
    state: &AppState,
    scan: &str,
    id: i64,
) -> Result<AnalysisContext, String> {
    snapshot_builder(state, scan)?.build(id).map_err(error)
}

pub(crate) fn snapshot_builder<'a>(
    state: &'a AppState,
    scan: &str,
) -> Result<context::ContextBuilder<'a>, String> {
    let settings = state.store.settings().map_err(error)?;
    let policy = cleaner_engine::safety::SafetyPolicy::new(settings);
    let (_, apps) = state.application_index(scan, &policy)?;
    context::ContextBuilder::with_index(&state.store, scan, apps).map_err(error)
}
pub(crate) fn analysis_config_hash(settings: &Settings, rule_version: &str) -> String {
    policy_config_hash(settings, client::config_hash(&settings.llm, rule_version))
}

fn policy_config_hash(settings: &Settings, client_config: String) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                client_config,
                &settings.protected_paths,
                &settings.unprotected_paths,
                &settings.ignored_paths,
                &settings.excluded_llm_paths,
                &settings.labels,
                &settings.history_reference_ids,
            ))
            .expect("serializable analysis policy")
        )
    )
}
#[tauri::command]
pub async fn llm_context(
    state: State<'_, Shared>,
    scan_id: String,
    entry_id: i64,
) -> Result<Value, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if !state.store.settings().map_err(error)?.llm.enabled {
            return Err("AI 未启用".into());
        }
        let context = snapshot_context(&state, &scan_id, entry_id)?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut previews = state.context_previews.lock().unwrap();
        previews.retain(|_, p| now() - p.created < 600);
        if previews.len() > 20 {
            previews.clear();
        }
        previews.insert(
            id.clone(),
            ContextPreview {
                context: context.clone(),
                created: now(),
            },
        );
        Ok(json!({"previewId":id,"context":context}))
    })
    .await
    .map_err(error)?
}
#[tauri::command]
pub async fn preview_samples(
    state: State<'_, Shared>,
    preview_id: String,
    entry_ids: Vec<i64>,
) -> Result<Value, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let context = {
            let previews = state.context_previews.lock().unwrap();
            let p = previews.get(&preview_id).ok_or("元数据预览不存在")?;
            if now() - p.created > 600 {
                return Err("预览已过期".into());
            }
            p.context.clone()
        };
        let samples =
            context::samples(&state.store, &context.scan_id, context.entry_id, &entry_ids)
                .map_err(error)?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut previews = state.sample_previews.lock().unwrap();
        previews.retain(|_, p| now() - p.created < 600);
        if previews.len() > 20 {
            previews.clear();
        }
        previews.insert(
            id.clone(),
            SamplePreview {
                context_id: preview_id,
                samples: samples.clone(),
                created: now(),
            },
        );
        Ok(json!({"previewId":id,"samples":samples}))
    })
    .await
    .map_err(error)?
}

#[derive(Debug)]
pub(crate) enum AnalysisOutcome {
    Completed,
    Cached,
    Failed(String),
}

pub(crate) fn run_one(
    state: &Shared,
    context: AnalysisContext,
    samples: Vec<Sample>,
    b: &Budget,
) -> Result<AnalysisOutcome, String> {
    run_one_with(
        state,
        context,
        samples,
        b,
        |settings, context, samples, budget| {
            let key = crate::settings_store::load_key(settings)?;
            client::analyze(settings, key.as_deref(), context, &json!(samples), budget)
        },
    )
}

fn run_one_with(
    state: &Shared,
    context: AnalysisContext,
    samples: Vec<Sample>,
    b: &Budget,
    send: impl FnOnce(
        &LlmSettings,
        &AnalysisContext,
        &[Sample],
        &Budget,
    ) -> anyhow::Result<client::Reply>,
) -> Result<AnalysisOutcome, String> {
    if b.cancel.load(Ordering::Relaxed) {
        return Err("分析已取消".into());
    }
    let settings = state.store.settings().map_err(error)?;
    if !settings.llm.enabled {
        return Err("AI 已关闭".into());
    }
    let rules = RuleSet::load(settings.community_enabled).map_err(error)?;
    let config = analysis_config_hash(&settings, &rules.version);
    let fresh = snapshot_context(state, &context.scan_id, context.entry_id)?;
    if context.fingerprint != fresh.fingerprint {
        return Err("分析输入或历史参考授权已变化，请重新预览".into());
    }
    context::validate_samples(&state.store, &fresh, &samples).map_err(error)?;
    if samples.is_empty()
        && state
            .store
            .analyses(&context.scan_id, context.entry_id)
            .map_err(error)?
            .iter()
            .any(|r| {
                r.status == "success"
                    && !r.included_content
                    && r.fingerprint == context.fingerprint
                    && r.config_hash == config
            })
    {
        let mut progress = state.progress.lock().unwrap();
        progress.finished += 1;
        progress.requests = b.requests.load(Ordering::Relaxed);
        return Ok(AnalysisOutcome::Cached);
    }
    if b.cancel.load(Ordering::Relaxed) {
        return Err("分析已取消".into());
    }
    state.progress.lock().unwrap().message = "正在等待 AI 返回…".into();
    let reply = send(&settings.llm, &context, &samples, b);
    let mut result = AnalysisResult {
        format_version: ANALYSIS_FORMAT_VERSION,
        id: uuid::Uuid::new_v4().to_string(),
        scan_id: context.scan_id.clone(),
        entry_id: context.entry_id,
        fingerprint: context.fingerprint.clone(),
        config_hash: config,
        created: now(),
        status: "failed".into(),
        message: String::new(),
        assessment: None,
        prompt_tokens: None,
        completion_tokens: None,
        included_content: !samples.is_empty(),
        evidence_details: context::evidence_details(&context),
        history_references: context.history_references.clone(),
        request_id: None,
        request_item_count: 1,
    };
    match reply {
        Ok(reply) => {
            result.status = "success".into();
            result.assessment = Some(reply.assessment);
            result.prompt_tokens = reply.prompt_tokens;
            result.completion_tokens = reply.completion_tokens;
            if b.cancel.load(Ordering::Relaxed)
                || !snapshot_context(state, &context.scan_id, context.entry_id)
                    .is_ok_and(|fresh| fresh.fingerprint == context.fingerprint)
                || context::validate_samples(&state.store, &context, &samples).is_err()
                || !state.store.settings().is_ok_and(|current| {
                    RuleSet::load(current.community_enabled).is_ok_and(|rules| {
                        analysis_config_hash(&current, &rules.version) == result.config_hash
                    })
                })
            {
                result.status = "stale".into();
                result.message = "分析期间扫描记录、授权范围或文本样本变化，结论已过期".into();
            }
        }
        Err(e) => {
            if let Some(failure) = e.downcast_ref::<client::BatchFailure>() {
                result.prompt_tokens = failure.prompt_tokens;
                result.completion_tokens = failure.completion_tokens;
            }
            result.message = error(e);
        }
    }
    state.store.save_analysis(&result).map_err(error)?;
    let mut p = state.progress.lock().unwrap();
    p.finished += 1;
    p.requests = b.requests.load(Ordering::Relaxed);
    Ok(if result.status == "success" {
        AnalysisOutcome::Completed
    } else {
        AnalysisOutcome::Failed(result.message)
    })
}
#[tauri::command]
pub fn analyze(
    state: State<'_, Shared>,
    preview_id: String,
    sample_preview_id: Option<String>,
) -> Result<(), String> {
    if state.analysis_busy.swap(true, Ordering::SeqCst) {
        return Err("已有 AI 分析任务运行，请等待或取消".into());
    }
    let setup = (|| -> Result<(AnalysisContext, Vec<Sample>, Budget), String> {
        let setup_guard = state.mutations.lock().unwrap();
        let p = state
            .context_previews
            .lock()
            .unwrap()
            .remove(&preview_id)
            .ok_or("预览不存在或已使用")?;
        if now() - p.created > 600 {
            return Err("元数据授权已过期".into());
        }
        let sample_preview = if let Some(id) = sample_preview_id {
            let s = state
                .sample_previews
                .lock()
                .unwrap()
                .remove(&id)
                .ok_or("正文授权不存在或已使用")?;
            if s.context_id != preview_id || now() - s.created > 600 {
                return Err("正文授权不属于本次分析或已过期".into());
            }
            Some(s)
        } else {
            None
        };
        let settings = state.store.settings().map_err(error)?;
        let b = budget(state.inner(), &p.context.scan_id, settings.llm.max_requests);
        b.cancel.store(false, Ordering::SeqCst);
        drop(setup_guard);
        let samples = if let Some(s) = sample_preview {
            let ids: Vec<_> = s.samples.iter().map(|s| s.entry_id).collect();
            let live = context::samples(&state.store, &p.context.scan_id, p.context.entry_id, &ids)
                .map_err(error)?;
            if live.len() != s.samples.len()
                || live
                    .iter()
                    .zip(&s.samples)
                    .any(|(a, b)| a.fingerprint != b.fingerprint || a.text != b.text)
            {
                return Err("授权后正文发生变化，请重新预览".into());
            }
            live
        } else {
            Vec::new()
        };
        Ok((p.context, samples, b))
    })();
    let (context, samples, b) = match setup {
        Ok(v) => v,
        Err(e) => {
            state.analysis_busy.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };
    *state.progress.lock().unwrap() = AnalysisProgress {
        scan_id: Some(context.scan_id.clone()),
        active: true,
        queued: 1,
        finished: 0,
        requests: b.requests.load(Ordering::Relaxed),
        max_requests: b.maximum,
        message: "手动分析".into(),
    };
    let state = state.inner().clone();
    std::thread::spawn(move || {
        let message = match run_one(&state, context, samples, &b) {
            Ok(AnalysisOutcome::Completed) => "AI 分析完成，结果在文件详情中查看。".into(),
            Ok(AnalysisOutcome::Cached) => "已有可用的 AI 分析结果，未重复请求。".into(),
            Ok(AnalysisOutcome::Failed(message)) | Err(message) => message,
        };
        let mut progress = state.progress.lock().unwrap();
        progress.message = message;
        progress.active = false;
        state.analysis_busy.store(false, Ordering::SeqCst);
    });
    Ok(())
}
#[tauri::command]
pub fn cancel_analysis(state: State<'_, Shared>) {
    let _guard = state.mutations.lock().unwrap();
    for b in state.budgets.lock().unwrap().values() {
        b.cancel.store(true, Ordering::SeqCst);
    }
    state.progress.lock().unwrap().message =
        "已请求取消；在途 HTTP 请求将在响应或超时后结束，不启动新请求".into();
}
#[tauri::command]
pub async fn test_connection(
    state: State<'_, Shared>,
    settings: LlmSettings,
    key: Option<String>,
) -> Result<String, String> {
    let stored = state.store.settings().map_err(error)?;
    let key = if key.is_some() {
        key
    } else if crate::settings_store::provider(&stored.llm.base_url).map_err(error)?
        == crate::settings_store::provider(&settings.base_url).map_err(error)?
    {
        crate::settings_store::load_key(&settings).map_err(error)?
    } else {
        None
    };
    tauri::async_runtime::spawn_blocking(move || {
        let r =
            cleaner_llm::connection::test_connection(&settings, key.as_deref()).map_err(error)?;
        let mut message = if r.reply_complete {
            "连接成功，模型已回复。".to_owned()
        } else {
            "连接成功，服务已接受测试请求。".to_owned()
        };
        if let (Some(input), Some(output)) = (r.prompt_tokens, r.completion_tokens) {
            message.push_str(&format!("输入 {input} token，输出 {output} token。"));
        }
        Ok(message)
    })
    .await
    .map_err(error)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_engine::store::Store;

    #[test]
    fn metadata_analysis_uses_the_finished_snapshot_without_reopening_the_target() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("index.sqlite")).unwrap();
        let path = temp.path().join("removed-source");
        assert!(!path.exists());
        let file = FileRecord {
            path: path.to_string_lossy().into_owned(),
            name: "removed-source".into(),
            is_dir: true,
            complete: true,
            file_count: 150_000,
            logical_bytes: 10_000_000_000,
            ..Default::default()
        };
        store
            .save_scan(&Scan {
                id: "s".into(),
                status: "complete".into(),
                root: temp.path().to_string_lossy().into_owned(),
                ..Default::default()
            })
            .unwrap();
        Store::insert_batch(
            &mut store.connection().unwrap(),
            "s",
            std::slice::from_ref(&file),
        )
        .unwrap();
        let file = store.by_path("s", &file.path).unwrap();
        let mut settings = Settings::default();
        settings.llm.enabled = true;
        store.put("settings", &settings).unwrap();
        let state = crate::state::AppState::new(store);
        let context = snapshot_context(&state, "s", file.id).unwrap();
        assert_eq!(context.file_count, 150_000);
        assert_eq!(context.logical_bytes, 10_000_000_000);
        assert!(context.note.contains("扫描记录"));
        let budget = budget(&state, "s", 10);
        let outcome = run_one_with(
            &state,
            context.clone(),
            vec![],
            &budget,
            |_, _, samples, b| {
                assert!(samples.is_empty());
                b.reserve()?;
                Ok(client::Reply {
                    assessment: ModelAssessment {
                        deletion_advice: DeletionAdvice::Review,
                        reason: "用途不明，需要人工核实".into(),
                        evidence: vec!["summary".into()],
                        history_matches: vec![],
                    },
                    prompt_tokens: Some(1),
                    completion_tokens: Some(1),
                })
            },
        )
        .unwrap();
        assert!(matches!(outcome, AnalysisOutcome::Completed));
        assert_eq!(state.progress.lock().unwrap().finished, 1);
        assert_eq!(state.store.get::<u32>("llm-budget:s").unwrap(), Some(1));
        assert_eq!(
            state.store.analyses("s", file.id).unwrap()[0].status,
            "success"
        );
        settings.llm.max_output_tokens = 16_384;
        state.store.put("settings", &settings).unwrap();
        assert!(matches!(
            run_one_with(&state, context.clone(), vec![], &budget, |_, _, _, _| {
                panic!("cached analysis must not send another request")
            })
            .unwrap(),
            AnalysisOutcome::Cached
        ));
        budget.cancel.store(true, Ordering::SeqCst);
        assert!(
            matches!(run_one(&state, context, vec![], &budget), Err(message) if message.contains("取消"))
        );
        assert_eq!(budget.requests.load(Ordering::SeqCst), 1);
    }
}

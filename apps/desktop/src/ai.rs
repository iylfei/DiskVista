use crate::state::{error, ContextPreview, SamplePreview, Shared};
use cleaner_domain::*;
use cleaner_engine::{
    cleanup::live_tree,
    context::{self, Sample},
    rules::RuleSet,
};
use cleaner_llm::client::{self, Budget};
use cleaner_platform::credentials;
use serde_json::{json, Value};
use std::sync::{atomic::Ordering, Arc};
use tauri::State;

fn now() -> i64 {
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
pub(crate) fn checked_context(
    state: &Shared,
    scan: &str,
    id: i64,
) -> Result<AnalysisContext, String> {
    let context = context::build(&state.store, scan, id).map_err(error)?;
    let f = state.store.entry(scan, id).map_err(error)?;
    if context::fingerprint(&live_tree(&f.path).map_err(error)?) != context.fingerprint {
        return Err("扫描后文件已变化，请重新扫描以获得新的分析输入".into());
    }
    Ok(context)
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
        let context = checked_context(&state, &scan_id, entry_id)?;
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
    let settings = state.store.settings().map_err(error)?;
    if !settings.llm.enabled {
        return Err("AI 已关闭".into());
    }
    let rules = RuleSet::load(settings.community_enabled).map_err(error)?;
    let config = client::config_hash(&settings.llm, &rules.version);
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
    let fresh = checked_context(state, &context.scan_id, context.entry_id)?;
    if context.fingerprint != fresh.fingerprint {
        return Err("分析输入已变化，请重新预览".into());
    }
    let key = credentials::load().map_err(error)?;
    let reply = client::analyze(&settings.llm, key.as_deref(), &context, &json!(samples), b);
    let mut result = AnalysisResult {
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
    };
    match reply {
        Ok(reply) => {
            result.status = "success".into();
            result.assessment = Some(reply.assessment);
            result.prompt_tokens = reply.prompt_tokens;
            result.completion_tokens = reply.completion_tokens;
            if checked_context(state, &context.scan_id, context.entry_id).is_err() {
                result.status = "stale".into();
                result.message = "分析期间目标变化，结论已过期".into();
            }
        }
        Err(e) => result.message = error(e),
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
        let p = state
            .context_previews
            .lock()
            .unwrap()
            .remove(&preview_id)
            .ok_or("预览不存在或已使用")?;
        if now() - p.created > 600 {
            return Err("元数据授权已过期".into());
        }
        let samples = if let Some(id) = sample_preview_id {
            let s = state
                .sample_previews
                .lock()
                .unwrap()
                .remove(&id)
                .ok_or("正文授权不存在或已使用")?;
            if s.context_id != preview_id || now() - s.created > 600 {
                return Err("正文授权不属于本次分析或已过期".into());
            }
            let ids: Vec<_> = s.samples.iter().map(|s| s.entry_id).collect();
            let live = context::samples(&state.store, &p.context.scan_id, p.context.entry_id, &ids)
                .map_err(error)?;
            if live
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
        let settings = state.store.settings().map_err(error)?;
        let b = budget(state.inner(), &p.context.scan_id, settings.llm.max_requests);
        b.cancel.store(false, Ordering::SeqCst);
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
    } else if stored.llm.base_url == settings.base_url {
        credentials::load().map_err(error)?
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

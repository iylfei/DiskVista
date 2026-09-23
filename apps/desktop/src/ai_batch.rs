use crate::{
    ai::{self, AnalysisOutcome},
    state::{error, Shared},
};
use cleaner_domain::{AnalysisContext, AnalysisResult, LlmSettings, ANALYSIS_FORMAT_VERSION};
use cleaner_engine::{
    classified_query::Classifier,
    context::{self, ContextBuilder},
    recycled_targets::RecycledTargets,
    rules::RuleSet,
    safety::SafetyPolicy,
};
use cleaner_llm::client::{self, Budget};
use std::{collections::HashMap, sync::atomic::Ordering};

pub(crate) fn run(
    state: &Shared,
    contexts: Vec<AnalysisContext>,
    budget: &Budget,
    automatic: bool,
) -> Result<Vec<Result<AnalysisOutcome, String>>, String> {
    run_with(
        state,
        contexts,
        budget,
        automatic,
        |settings, contexts, budget| {
            let key = crate::settings_store::load_key(settings)?;
            cleaner_llm::analyze_batch(settings, key.as_deref(), contexts, budget)
        },
    )
}

fn run_with(
    state: &Shared,
    contexts: Vec<AnalysisContext>,
    budget: &Budget,
    automatic: bool,
    send: impl FnOnce(
        &LlmSettings,
        &[AnalysisContext],
        &Budget,
    ) -> anyhow::Result<cleaner_llm::BatchReply>,
) -> Result<Vec<Result<AnalysisOutcome, String>>, String> {
    if contexts.is_empty() {
        return Ok(Vec::new());
    }
    if budget.cancel.load(Ordering::Relaxed) {
        return Err("分析已取消".into());
    }
    let settings = state.store.settings().map_err(error)?;
    if !authorized(&settings.llm, automatic) {
        return Err("AI 分析授权已关闭".into());
    }
    let scan_id = &contexts[0].scan_id;
    cleaner_llm::validate_batch_contexts(&contexts).map_err(error)?;
    let rules = RuleSet::load(settings.community_enabled).map_err(error)?;
    let config = ai::analysis_config_hash(&settings, &rules.version);
    let policy = SafetyPolicy::new(settings.clone());
    let (_, applications) = state.application_index(scan_id, &policy)?;
    let scan = state.store.require_finished(scan_id).map_err(error)?;
    let recycled = RecycledTargets::load(&state.store, &scan).map_err(error)?;
    let classifier = Classifier::new(&scan, &rules, &policy, &applications);
    let builder =
        ContextBuilder::with_index(&state.store, scan_id, applications.clone()).map_err(error)?;
    let ids: Vec<_> = contexts.iter().map(|context| context.entry_id).collect();
    let existing = state
        .store
        .analyses_for_entries(scan_id, &ids)
        .map_err(error)?;
    let mut outcomes = HashMap::new();
    let mut pending = Vec::new();
    let mut paths = HashMap::new();
    for context in &contexts {
        let file = state.store.entry(scan_id, context.entry_id);
        let removed = file
            .as_ref()
            .is_ok_and(|file| recycled.contains(&file.path));
        let eligible = file.is_ok_and(|mut file| {
            paths.insert(context.entry_id, file.path.clone());
            classifier.apply(&mut file);
            crate::scan_analysis::eligible(
                &file,
                &policy,
                &applications,
                settings.llm.minimum_bytes,
            )
        });
        if removed {
            outcomes.insert(context.entry_id, Err("文件已移入回收站，已跳过".into()));
        } else if !eligible {
            outcomes.insert(
                context.entry_id,
                Err("文件已不符合批量分析的来源或大小范围，已跳过".into()),
            );
        } else if !builder
            .build(context.entry_id)
            .is_ok_and(|fresh| fresh.fingerprint == context.fingerprint)
        {
            outcomes.insert(
                context.entry_id,
                Err("分析输入或授权范围已变化，已跳过".into()),
            );
        } else if existing.iter().any(|result| {
            result.entry_id == context.entry_id
                && result.status == "success"
                && result.assessment.is_some()
                && !result.included_content
                && result.fingerprint == context.fingerprint
                && result.config_hash == config
        }) {
            outcomes.insert(context.entry_id, Ok(AnalysisOutcome::Cached));
        } else {
            pending.push(context.clone());
        }
    }
    let recycled = RecycledTargets::load(&state.store, &scan).map_err(error)?;
    pending.retain(|context| {
        if paths
            .get(&context.entry_id)
            .is_some_and(|path| recycled.contains(path))
        {
            outcomes.insert(context.entry_id, Err("文件已移入回收站，已跳过".into()));
            false
        } else {
            true
        }
    });
    if !pending.is_empty() {
        let current = state.store.settings().map_err(error)?;
        let current_rules = RuleSet::load(current.community_enabled).map_err(error)?;
        if budget.cancel.load(Ordering::Relaxed)
            || !authorized(&current.llm, automatic)
            || ai::analysis_config_hash(&current, &current_rules.version) != config
        {
            return Err("分析设置或授权已变化，未发送这批文件".into());
        }
        if pending.len() > crate::scan_analysis::grouping::item_limit(&current.llm) {
            return Err("输出上限已变化，请重新开始以调整每批文件数量".into());
        }
        state.progress.lock().unwrap().message = format!("正在分析本批 {} 个文件…", pending.len());
        let reply = send(&current.llm, &pending, budget);
        let (mut answers, prompt_tokens, completion_tokens) = match reply {
            Ok(reply) => (
                reply
                    .items
                    .into_iter()
                    .map(|item| (item.entry_id, item.assessment))
                    .collect::<HashMap<_, _>>(),
                reply.prompt_tokens,
                reply.completion_tokens,
            ),
            Err(failure) => {
                if failure.is::<client::BudgetExhausted>() {
                    return Err(error(failure));
                }
                let usage = failure.downcast_ref::<client::BatchFailure>();
                let prompt = usage.and_then(|failure| failure.prompt_tokens);
                let completion = usage.and_then(|failure| failure.completion_tokens);
                let message = error(failure);
                (
                    pending
                        .iter()
                        .map(|context| (context.entry_id, Err(message.clone())))
                        .collect(),
                    prompt,
                    completion,
                )
            }
        };
        let final_settings = state.store.settings().map_err(error)?;
        let final_recycled = RecycledTargets::load(&state.store, &scan).map_err(error)?;
        let final_rules = RuleSet::load(final_settings.community_enabled).map_err(error)?;
        let unchanged = !budget.cancel.load(Ordering::Relaxed)
            && authorized(&final_settings.llm, automatic)
            && ai::analysis_config_hash(&final_settings, &final_rules.version) == config;
        let final_policy = SafetyPolicy::new(final_settings);
        let final_builder = state
            .application_index(scan_id, &final_policy)
            .ok()
            .and_then(|(_, apps)| ContextBuilder::with_index(&state.store, scan_id, apps).ok());
        let request_id = uuid::Uuid::new_v4().to_string();
        let mut results = Vec::new();
        for (index, context) in pending.iter().enumerate() {
            let answer = answers
                .remove(&context.entry_id)
                .unwrap_or_else(|| Err("AI 未返回此文件的建议".into()));
            let mut result = AnalysisResult {
                format_version: ANALYSIS_FORMAT_VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                scan_id: context.scan_id.clone(),
                entry_id: context.entry_id,
                fingerprint: context.fingerprint.clone(),
                config_hash: config.clone(),
                created: ai::now(),
                status: "failed".into(),
                message: String::new(),
                assessment: None,
                prompt_tokens: if index == 0 { prompt_tokens } else { None },
                completion_tokens: if index == 0 { completion_tokens } else { None },
                request_id: Some(request_id.clone()),
                request_item_count: pending.len() as u32,
                included_content: false,
                evidence_details: context::evidence_details(context),
                history_references: context.history_references.clone(),
            };
            match answer {
                Ok(assessment) => {
                    result.status = "success".into();
                    result.assessment = Some(assessment);
                    if paths
                        .get(&context.entry_id)
                        .is_some_and(|path| final_recycled.contains(path))
                    {
                        result.status = "stale".into();
                        result.message = "分析期间文件已移入回收站，结论已过期".into();
                    } else if !unchanged
                        || !final_builder.as_ref().is_some_and(|builder| {
                            builder
                                .build(context.entry_id)
                                .is_ok_and(|fresh| fresh.fingerprint == context.fingerprint)
                        })
                    {
                        result.status = "stale".into();
                        result.message = "分析期间扫描记录、授权范围或配置变化，结论已过期".into();
                    }
                }
                Err(message) => result.message = message,
            }
            outcomes.insert(
                context.entry_id,
                Ok(if result.status == "success" {
                    AnalysisOutcome::Completed
                } else {
                    AnalysisOutcome::Failed(result.message.clone())
                }),
            );
            results.push(result);
        }
        state.store.save_analyses(&results).map_err(error)?;
    }
    let mut progress = state.progress.lock().unwrap();
    progress.finished += contexts.len() as u32;
    progress.requests = budget.requests.load(Ordering::Relaxed);
    Ok(ids
        .into_iter()
        .map(|id| {
            outcomes
                .remove(&id)
                .expect("each batch item has an outcome")
        })
        .collect())
}

fn authorized(settings: &LlmSettings, automatic: bool) -> bool {
    settings.enabled && (!automatic || (settings.automatic && settings.metadata_consent))
}

#[cfg(test)]
mod tests;

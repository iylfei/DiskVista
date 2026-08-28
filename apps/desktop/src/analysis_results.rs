use crate::state::{error, AppState, Shared};
use cleaner_domain::{AnalysisResult, AnalysisSummary};
use cleaner_engine::analysis_filter::AnalysisFilter;
use cleaner_engine::{context::ContextBuilder, rules::RuleSet};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use tauri::State;

fn reconcile(
    results: &mut [AnalysisResult],
    config: &str,
    revalidate: bool,
    check: impl FnOnce() -> Option<String>,
) -> Vec<usize> {
    let current = if revalidate && results.iter().any(|r| r.status == "success") {
        Some(check())
    } else {
        None
    };
    let mut changed = Vec::new();
    for (i, result) in results.iter_mut().enumerate() {
        if result.status == "success"
            && (result.config_hash != config
                || current
                    .as_ref()
                    .is_some_and(|fp| fp.as_ref() != Some(&result.fingerprint)))
        {
            result.status = "stale".into();
            result.message =
                "扫描记录、相关回收历史、授权范围、规则或模型配置变化，结果已过期".into();
            changed.push(i);
        }
    }
    changed
}

#[tauri::command]
pub async fn analysis_results(
    state: State<'_, Shared>,
    scan_id: String,
    entry_id: i64,
    revalidate: Option<bool>,
) -> Result<Vec<AnalysisResult>, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        let mut results = state.store.analyses(&scan_id, entry_id).map_err(error)?;
        if results.iter().any(|r| r.status == "success") {
            let settings = state.store.settings().map_err(error)?;
            let rules = RuleSet::load(settings.community_enabled).map_err(error)?;
            let config = crate::ai::analysis_config_hash(&settings, &rules.version);
            let changed = reconcile(&mut results, &config, revalidate.unwrap_or(true), || {
                crate::ai::snapshot_context(&state, &scan_id, entry_id)
                    .ok()
                    .map(|c| c.fingerprint)
            });
            for i in changed {
                state.store.save_analysis(&results[i]).map_err(error)?;
            }
        }
        state
            .store
            .hydrate_batch_usage(&mut results)
            .map_err(error)?;
        Ok(results)
    })
    .await
}

pub(crate) fn summaries(
    state: &AppState,
    scan: &str,
    ids: &[i64],
) -> Result<Vec<AnalysisSummary>, String> {
    Ok(current_results(state, scan, ids)?
        .into_values()
        .filter_map(|results| {
            results
                .iter()
                .find(|result| result.status == "success" && result.assessment.is_some())
                .or_else(|| {
                    results
                        .iter()
                        .find(|result| result.status == "stale" && result.assessment.is_some())
                })
                .map(summary)
        })
        .collect())
}

fn current_results(
    state: &AppState,
    scan: &str,
    ids: &[i64],
) -> Result<BTreeMap<i64, Vec<AnalysisResult>>, String> {
    if ids.len() > 200 || ids.iter().any(|id| *id <= 0) {
        return Err("请提供最多 200 个有效文件 ID".into());
    }
    let mut grouped: BTreeMap<i64, Vec<AnalysisResult>> = BTreeMap::new();
    for result in state.store.analyses_for_entries(scan, ids).map_err(error)? {
        grouped.entry(result.entry_id).or_default().push(result);
    }
    if grouped.is_empty() {
        return Ok(grouped);
    }
    let settings = state.store.settings().map_err(error)?;
    let rules = RuleSet::load(settings.community_enabled).map_err(error)?;
    let config = crate::ai::analysis_config_hash(&settings, &rules.version);
    let builder = if grouped
        .values()
        .flatten()
        .any(|r| r.status == "success" && r.config_hash == config)
    {
        let policy = cleaner_engine::safety::SafetyPolicy::new(settings.clone());
        state
            .application_index(scan, &policy)
            .ok()
            .and_then(|(_, apps)| ContextBuilder::with_index(&state.store, scan, apps).ok())
    } else {
        None
    };
    let mut changed = Vec::new();
    for (entry_id, results) in &mut grouped {
        let updates = reconcile(results, &config, true, || {
            builder
                .as_ref()?
                .build(*entry_id)
                .ok()
                .map(|context| context.fingerprint)
        });
        changed.extend(updates.into_iter().map(|index| results[index].clone()));
    }
    state.store.save_analyses(&changed).map_err(error)?;
    Ok(grouped)
}

pub(crate) fn reusable_ids(
    state: &AppState,
    scan: &str,
    ids: &[i64],
) -> Result<BTreeSet<i64>, String> {
    valid_ids(state, scan, ids, true)
}

fn valid_ids(
    state: &AppState,
    scan: &str,
    ids: &[i64],
    metadata_only: bool,
) -> Result<BTreeSet<i64>, String> {
    let mut valid = BTreeSet::new();
    for chunk in ids.chunks(200) {
        for (id, results) in current_results(state, scan, chunk)? {
            if results.iter().any(|result| {
                result.status == "success"
                    && result.assessment.is_some()
                    && (!metadata_only || !result.included_content)
            }) {
                valid.insert(id);
            }
        }
    }
    Ok(valid)
}

pub(crate) fn filter(
    state: &AppState,
    scan: &str,
    status: &str,
) -> Result<Option<AnalysisFilter>, String> {
    if status.is_empty() {
        return Ok(None);
    }
    AnalysisFilter::new(status, BTreeSet::new()).map_err(error)?;
    let ids = state.store.analysis_entry_ids(scan).map_err(error)?;
    AnalysisFilter::new(status, valid_ids(state, scan, &ids, false)?).map_err(error)
}

fn summary(result: &AnalysisResult) -> AnalysisSummary {
    let assessment = result.assessment.as_ref();
    let active = result.status == "success";
    let ids: HashSet<_> = result
        .history_references
        .iter()
        .map(|reference| &reference.id)
        .collect();
    let history_match_count = assessment.filter(|_| active).map_or(0, |assessment| {
        assessment
            .history_matches
            .iter()
            .filter(|matched| {
                ids.contains(&matched.history_id)
                    && assessment.evidence.contains(&matched.history_id)
            })
            .map(|matched| &matched.history_id)
            .collect::<HashSet<_>>()
            .len()
    });
    AnalysisSummary {
        analysis_id: result.id.clone(),
        entry_id: result.entry_id,
        created: result.created,
        status: result.status.clone(),
        summary: assessment
            .filter(|_| active)
            .map(|value| format!("{}：{}", value.deletion_advice.label(), value.reason))
            .unwrap_or_default(),
        history_match_count,
    }
}

#[tauri::command]
pub async fn analysis_summaries(
    state: State<'_, Shared>,
    scan_id: String,
    entry_ids: Vec<i64>,
) -> Result<Vec<AnalysisSummary>, String> {
    let state = state.inner().clone();
    crate::background::read(move || summaries(&state, &scan_id, &entry_ids)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use cleaner_domain::{Assessment, FileRecord, HistoryItem, ModelAssessment, Scan, Settings};
    use cleaner_engine::store::Store;
    fn result() -> AnalysisResult {
        AnalysisResult {
            format_version: cleaner_domain::ANALYSIS_FORMAT_VERSION,
            id: "r".into(),
            scan_id: "s".into(),
            entry_id: 1,
            fingerprint: "same".into(),
            config_hash: "config".into(),
            created: 0,
            status: "success".into(),
            message: String::new(),
            assessment: None,
            prompt_tokens: None,
            completion_tokens: None,
            request_id: None,
            request_item_count: 1,
            included_content: false,
            evidence_details: vec![],
            history_references: vec![],
        }
    }
    #[test]
    fn polling_never_enumerates_and_still_invalidates_changed_configuration() {
        let mut rows = vec![result()];
        assert!(reconcile(&mut rows, "config", false, || panic!(
            "unexpected filesystem read"
        ))
        .is_empty());
        assert_eq!(
            reconcile(&mut rows, "changed", false, || panic!(
                "unexpected filesystem read"
            )),
            vec![0]
        );
    }
    #[test]
    fn explicit_checks_fail_closed_and_do_not_recheck_stale_results() {
        for fingerprint in [None, Some("different".into())] {
            let mut rows = vec![result()];
            assert_eq!(
                reconcile(&mut rows, "config", true, || fingerprint),
                vec![0]
            );
            assert!(reconcile(&mut rows, "config", true, || panic!("already stale")).is_empty());
        }
        assert!(reconcile(&mut [result()], "config", true, || Some("same".into())).is_empty());
    }

    fn model(reason: &str) -> ModelAssessment {
        ModelAssessment {
            deletion_advice: cleaner_domain::DeletionAdvice::Review,
            reason: reason.into(),
            evidence: vec!["summary".into()],
            history_matches: vec![],
        }
    }

    #[test]
    fn batch_summaries_validate_history_scope_and_never_expose_stale_suggestions() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path().join("index.sqlite")).unwrap();
        let file = FileRecord {
            path: "D:\\analysis-fixture\\Example\\editorbundle-2.zip".into(),
            parent: "D:\\analysis-fixture\\Example".into(),
            name: "editorbundle-2.zip".into(),
            complete: true,
            logical_bytes: 200_000_000,
            file_count: 1,
            assessment: Assessment {
                risk: "review".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        store
            .save_scan(&Scan {
                id: "s".into(),
                root: "D:\\analysis-fixture".into(),
                status: "complete".into(),
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
        settings.llm.history_reference_enabled = true;
        store.put("settings", &settings).unwrap();
        store
            .add_history(&HistoryItem {
                id: "h".into(),
                batch_id: "b".into(),
                path: "D:\\analysis-fixture\\Example\\editorbundle-1.zip".into(),
                bytes: 100,
                time: chrono::Utc::now().timestamp() - 10,
                status: "recycled".into(),
                message: String::new(),
                free_space_delta: 0,
                snapshot: None,
            })
            .unwrap();
        let state = crate::state::AppState::new(store);
        let context = crate::ai::snapshot_context(&state, "s", file.id).unwrap();
        assert_eq!(context.history_references.len(), 1);
        let mut row = result();
        row.entry_id = file.id;
        row.fingerprint = context.fingerprint;
        row.config_hash =
            crate::ai::analysis_config_hash(&settings, &RuleSet::load(false).unwrap().version);
        row.assessment = Some(model("与之前回收的安装包相似，先确认是否仍需保留"));
        let assessment = row.assessment.as_mut().unwrap();
        assessment.evidence.push("history:h".into());
        assessment
            .history_matches
            .push(cleaner_domain::HistoryMatch {
                history_id: "history:h".into(),
                reason: "相同目录、同产品名称的不同版本".into(),
            });
        row.history_references = context.history_references;
        state.store.save_analysis(&row).unwrap();
        let current = summaries(&state, "s", &[file.id, file.id]).unwrap();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].status, "success");
        assert_eq!(current[0].history_match_count, 1);
        assert!(filter(&state, "s", "analyzed")
            .unwrap()
            .unwrap()
            .matches(file.id));
        assert!(!filter(&state, "s", "unanalyzed")
            .unwrap()
            .unwrap()
            .matches(file.id));
        assert!(summaries(&state, "different-scan", &[file.id])
            .unwrap()
            .is_empty());
        assert!(summaries(&state, "s", &vec![file.id; 201]).is_err());
        settings.llm.history_reference_enabled = false;
        state.store.put("settings", &settings).unwrap();
        let stale = summaries(&state, "s", &[file.id]).unwrap();
        assert_eq!(stale[0].status, "stale");
        assert_eq!(stale[0].history_match_count, 0);
        assert!(stale[0].summary.is_empty());
        assert!(!filter(&state, "s", "analyzed")
            .unwrap()
            .unwrap()
            .matches(file.id));
        assert!(filter(&state, "s", "unanalyzed")
            .unwrap()
            .unwrap()
            .matches(file.id));
        assert_eq!(
            state.store.analyses("s", file.id).unwrap()[0].status,
            "stale"
        );
    }

    #[test]
    fn unbacked_history_cannot_create_a_badge() {
        let mut row = result();
        row.assessment = Some(model("用途不明，需要人工核实"));
        row.assessment
            .as_mut()
            .unwrap()
            .history_matches
            .push(cleaner_domain::HistoryMatch {
                history_id: "history:invented".into(),
                reason: "没有对应输入".into(),
            });
        assert_eq!(summary(&row).history_match_count, 0);
    }
}

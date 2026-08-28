use crate::state::{error, Shared};
use cleaner_domain::AnalysisResult;
use cleaner_engine::rules::RuleSet;
use cleaner_llm::client;
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
            result.message = "扫描记录、授权范围、规则或模型配置变化，结果已过期".into();
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
            let config = client::config_hash(&settings.llm, &rules.version);
            let changed = reconcile(&mut results, &config, revalidate.unwrap_or(true), || {
                crate::ai::snapshot_context(&state, &scan_id, entry_id)
                    .ok()
                    .map(|c| c.fingerprint)
            });
            for i in changed {
                state.store.save_analysis(&results[i]).map_err(error)?;
            }
        }
        Ok(results)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    fn result() -> AnalysisResult {
        AnalysisResult {
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
            included_content: false,
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
}

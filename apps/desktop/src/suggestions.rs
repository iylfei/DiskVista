use crate::state::{error, Shared};
use cleaner_domain::{FileRecord, SuggestionPage, SuggestionQuery};
use cleaner_engine::{
    safety::SafetyPolicy,
    suggestions::{self, SuggestionIndex},
};
use tauri::State;

fn with_index<T>(
    state: &Shared,
    scan: &str,
    read: impl FnOnce(&SuggestionIndex) -> anyhow::Result<T>,
) -> Result<T, String> {
    state.store.require_finished(scan).map_err(error)?;
    let policy = SafetyPolicy::new(state.store.settings().map_err(error)?);
    let (key, applications) = state.application_index(scan, &policy)?;
    let mut cached = state.suggestions_snapshot.lock().unwrap();
    for _ in 0..3 {
        if cached
            .as_ref()
            .is_some_and(|(old, index)| old == &key && index.is_current())
        {
            return read(&cached.as_ref().unwrap().1).map_err(error);
        }
        let index = suggestions::build_indexed(&state.store, scan, &policy, &applications)
            .map_err(error)?;
        *cached = Some((key.clone(), index));
    }
    Err("分类依据已变化，请重新加载".into())
}

#[tauri::command]
pub async fn cleanup_suggestions(
    state: State<'_, Shared>,
    query: SuggestionQuery,
) -> Result<SuggestionPage, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        let analysis =
            crate::analysis_results::filter(&state, &query.scan_id, &query.analysis_status)?;
        with_index(&state, &query.scan_id, |index| {
            suggestions::page_with_analysis(index, &query, analysis.as_ref())
        })
    })
    .await
}

#[tauri::command]
pub async fn select_suggestion_group(
    state: State<'_, Shared>,
    query: SuggestionQuery,
) -> Result<Vec<FileRecord>, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        let analysis =
            crate::analysis_results::filter(&state, &query.scan_id, &query.analysis_status)?;
        with_index(&state, &query.scan_id, |index| {
            suggestions::selection_with_analysis(index, &query, analysis.as_ref())
        })
    })
    .await
}

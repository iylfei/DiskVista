use crate::state::{error, Shared};
use cleaner_domain::{FileRecord, SuggestionPage, SuggestionQuery};
use cleaner_engine::suggestions::{self, SuggestionIndex};
use tauri::State;

fn with_index<T>(
    state: &Shared,
    scan: &str,
    read: impl FnOnce(&SuggestionIndex) -> anyhow::Result<T>,
) -> Result<T, String> {
    state.store.require_finished(scan).map_err(error)?;
    let key = format!(
        "{}:{}",
        scan,
        serde_json::to_string(&state.store.settings().map_err(error)?).map_err(error)?
    );
    let mut cached = state.suggestions_snapshot.lock().unwrap();
    if cached.as_ref().is_none_or(|(old, _)| old != &key) {
        *cached = Some((key, suggestions::build(&state.store, scan).map_err(error)?));
    }
    read(&cached.as_ref().unwrap().1).map_err(error)
}

#[tauri::command]
pub async fn cleanup_suggestions(
    state: State<'_, Shared>,
    query: SuggestionQuery,
) -> Result<SuggestionPage, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        with_index(&state, &query.scan_id, |index| {
            Ok(suggestions::page(index, &query))
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
        with_index(&state, &query.scan_id, |index| {
            suggestions::selection(index, &query)
        })
    })
    .await
}

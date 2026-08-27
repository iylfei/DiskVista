use crate::state::{error, Shared};
use cleaner_domain::*;
use cleaner_engine::cleanup;
use std::sync::atomic::Ordering;
use tauri::State;
#[tauri::command]
pub async fn preview_cleanup(
    state: State<'_, Shared>,
    scan_id: String,
    entry_ids: Vec<i64>,
) -> Result<CleanupPreview, String> {
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let p = cleanup::preview(&shared.store, &scan_id, &entry_ids).map_err(error)?;
        let mut previews = shared.cleanup_previews.lock().unwrap();
        previews.retain(|_, p| chrono::Utc::now().timestamp() - p.created < 600);
        if previews.len() > 20 {
            previews.clear();
        }
        previews.insert(p.id.clone(), p.clone());
        Ok(p)
    })
    .await
    .map_err(error)?
}
#[tauri::command]
pub async fn execute_cleanup(
    state: State<'_, Shared>,
    preview_id: String,
    acknowledge_risk: bool,
) -> Result<Vec<HistoryItem>, String> {
    if state.cleaning.swap(true, Ordering::SeqCst) {
        return Err("已有回收操作正在运行".into());
    }
    state.cleanup_cancel.store(false, Ordering::SeqCst);
    let preview = state.cleanup_previews.lock().unwrap().remove(&preview_id);
    let Some(preview) = preview else {
        state.cleaning.store(false, Ordering::SeqCst);
        return Err("预览不存在、已过期或已使用".into());
    };
    let shared = state.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        cleanup::execute(
            &shared.store,
            &preview,
            acknowledge_risk,
            shared.cleanup_cancel.clone(),
        )
        .map_err(error)
    })
    .await
    .map_err(error);
    state.cleaning.store(false, Ordering::SeqCst);
    result?
}
#[tauri::command]
pub fn cancel_cleanup(state: State<'_, Shared>) {
    state.cleanup_cancel.store(true, Ordering::SeqCst);
}
#[tauri::command]
pub async fn history(state: State<'_, Shared>) -> Result<Vec<HistoryItem>, String> {
    let state = state.inner().clone();
    crate::background::read(move || state.store.history().map_err(error)).await
}
#[tauri::command]
pub fn open_location(
    state: State<'_, Shared>,
    scan_id: String,
    entry_id: i64,
) -> Result<(), String> {
    let f = state.store.entry(&scan_id, entry_id).map_err(error)?;
    cleaner_platform::shell::reveal(&f.path).map_err(error)
}
#[tauri::command]
pub fn open_system(target: String) -> Result<(), String> {
    cleaner_platform::shell::open_system(&target).map_err(error)
}

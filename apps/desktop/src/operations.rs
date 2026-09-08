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
    request_id: Option<String>,
) -> Result<CleanupPreview, String> {
    let request_id = request_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let check = state.cleanup_checks.begin(&request_id, entry_ids.len())?;
    let shared = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            // A cancelled preview must not wait for a long mutation to release its lock.
            let _guard = loop {
                if check.cancel.load(Ordering::Relaxed) {
                    return Err("安全检查已取消".into());
                }
                match shared.mutations.try_lock() {
                    Ok(guard) => break guard,
                    Err(std::sync::TryLockError::WouldBlock) => {
                        std::thread::sleep(std::time::Duration::from_millis(20))
                    }
                    Err(_) => return Err("无法获取安全检查锁".into()),
                }
            };
            cleanup::preview_with_progress(
                &shared.store,
                &scan_id,
                &entry_ids,
                check.cancel.clone(),
                |progress| check.report(progress),
            )
            .map_err(error)
        })();
        check.finish(result, &shared.cleanup_previews)
    })
    .await
    .map_err(error)?
}
#[tauri::command]
pub fn cleanup_check_progress(
    state: State<'_, Shared>,
    request_id: String,
) -> Option<CleanupCheckProgress> {
    state.cleanup_checks.progress(&request_id)
}
#[tauri::command]
pub fn cancel_cleanup_check(state: State<'_, Shared>, request_id: String) -> Result<(), String> {
    state
        .cleanup_checks
        .cancel(&request_id, &state.cleanup_previews)
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
        let _guard = shared.mutations.lock().unwrap();
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
pub async fn history_page(
    state: State<'_, Shared>,
    offset: Option<u64>,
    limit: Option<u32>,
) -> Result<HistoryPage, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        state
            .store
            .history_page(offset.unwrap_or(0), limit.unwrap_or(20))
            .map_err(error)
    })
    .await
}
#[tauri::command]
pub async fn open_location(
    state: State<'_, Shared>,
    scan_id: String,
    entry_id: i64,
) -> Result<(), String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        let f = state.store.entry(&scan_id, entry_id).map_err(error)?;
        cleaner_platform::shell::reveal(&f.path).map_err(error)
    })
    .await
}
#[tauri::command]
pub fn open_system(target: String) -> Result<(), String> {
    cleaner_platform::shell::open_system(&target).map_err(error)
}

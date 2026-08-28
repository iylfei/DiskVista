use crate::state::{error, Shared};
use cleaner_domain::{AnalysisProgress, Scan};
use std::sync::atomic::Ordering;
use tauri::State;

#[tauri::command]
pub async fn delete_scan(state: State<'_, Shared>, scan_id: String) -> Result<Vec<Scan>, String> {
    let state = state.inner().clone();
    crate::background::read(move || remove(&state, &scan_id)).await
}

fn remove(state: &Shared, scan_id: &str) -> Result<Vec<Scan>, String> {
    let _guard = state.mutations.lock().unwrap();
    if state.worker.lock().unwrap().is_some()
        || state.cleaning.load(Ordering::SeqCst)
        || state.analysis_busy.load(Ordering::SeqCst)
    {
        return Err("扫描、AI 分析或回收运行期间不能删除扫描记录".into());
    }
    let remaining = state.store.delete_scan(scan_id).map_err(error)?;
    state.invalidate_classification();
    state
        .cleanup_previews
        .lock()
        .unwrap()
        .retain(|_, value| value.scan_id != scan_id);
    state
        .context_previews
        .lock()
        .unwrap()
        .retain(|_, value| value.context.scan_id != scan_id);
    state.sample_previews.lock().unwrap().clear();
    state.budgets.lock().unwrap().remove(scan_id);
    let mut progress = state.progress.lock().unwrap();
    if progress.scan_id.as_deref() == Some(scan_id) {
        *progress = AnalysisProgress::default();
    }
    drop(progress);
    Ok(remaining)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use cleaner_engine::store::Store;

    #[test]
    fn active_analysis_and_cleanup_block_deletion_and_idle_deletion_clears_progress() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path().join("db.sqlite")).unwrap();
        store
            .save_scan(&Scan {
                id: "s".into(),
                status: "complete".into(),
                ..Default::default()
            })
            .unwrap();
        let state = AppState::new(store);
        state.analysis_busy.store(true, Ordering::SeqCst);
        assert!(remove(&state, "s").is_err());
        state.analysis_busy.store(false, Ordering::SeqCst);
        state.cleaning.store(true, Ordering::SeqCst);
        assert!(remove(&state, "s").is_err());
        state.cleaning.store(false, Ordering::SeqCst);
        state.progress.lock().unwrap().scan_id = Some("s".into());
        assert!(remove(&state, "s").unwrap().is_empty());
        assert!(state.progress.lock().unwrap().scan_id.is_none());
    }
}

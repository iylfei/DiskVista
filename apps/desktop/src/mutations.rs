use crate::state::{classification_key, error, Shared};
use cleaner_domain::Settings;
use cleaner_platform::credentials;
use std::sync::atomic::Ordering;
use tauri::State;

#[tauri::command]
pub async fn save_settings(
    state: State<'_, Shared>,
    mut settings: Settings,
    key: Option<String>,
) -> Result<Settings, String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        let _guard = state.mutations.lock().unwrap();
        if state.cleaning.load(Ordering::SeqCst) {
            return Err("回收执行期间不能更改安全设置".into());
        }
        let previous = state.store.settings().map_err(error)?;
        if !settings.llm.base_url.is_empty() {
            cleaner_llm::client::endpoint(&settings.llm.base_url).map_err(error)?;
        }
        settings.scan_retention = if settings.scan_retention == 0 {
            0
        } else {
            settings.scan_retention.clamp(2, 1000)
        };
        settings.llm.max_requests = settings.llm.max_requests.clamp(1, 100);
        settings.llm.concurrency = settings.llm.concurrency.clamp(1, 2);
        settings.llm.timeout_seconds = settings.llm.timeout_seconds.clamp(5, 300);
        settings.llm.minimum_bytes = settings.llm.minimum_bytes.max(1048576);
        if previous.llm.base_url != settings.llm.base_url {
            credentials::clear().map_err(error)?;
            settings.llm.metadata_consent = false;
        }
        if !settings.llm.enabled || previous.llm.base_url != settings.llm.base_url {
            for budget in state.budgets.lock().unwrap().values() {
                budget.cancel.store(true, Ordering::SeqCst);
            }
            state.context_previews.lock().unwrap().clear();
            state.sample_previews.lock().unwrap().clear();
        }
        if let Some(key) = key {
            credentials::save(&key).map_err(error)?;
        }
        state.store.put("settings", &settings).map_err(error)?;
        if classification_key(&previous)? != classification_key(&settings)? {
            state.invalidate_classification();
        }
        Ok(settings)
    })
    .await
}

#[tauri::command]
pub async fn set_annotation(
    state: State<'_, Shared>,
    scan_id: String,
    entry_id: i64,
    kind: String,
    value: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    crate::background::read(move || {
        let _guard = state.mutations.lock().unwrap();
        if state.cleaning.load(Ordering::SeqCst) {
            return Err("回收执行期间不能更改安全设置".into());
        }
        let result =
            cleaner_engine::annotations::apply(&state.store, &scan_id, entry_id, &kind, &value)
                .map_err(error);
        if kind != "exclude_llm" {
            state.invalidate_classification();
        }
        result
    })
    .await
}

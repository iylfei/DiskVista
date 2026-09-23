use crate::state::{classification_key, error, Shared};
use cleaner_domain::Settings;
use std::sync::atomic::Ordering;
use tauri::State;

fn update_ai_consent(previous: &Settings, settings: &mut Settings) -> bool {
    let changed_provider = crate::settings_store::provider(&previous.llm.base_url).ok()
        != crate::settings_store::provider(&settings.llm.base_url).ok();
    if changed_provider {
        settings.llm.metadata_consent = false;
        settings.llm.history_reference_enabled = false;
    }
    !settings.llm.enabled
        || previous.llm != settings.llm
        || previous.history_reference_ids != settings.history_reference_ids
        || previous.community_enabled != settings.community_enabled
        || previous.labels != settings.labels
        || previous.excluded_llm_paths != settings.excluded_llm_paths
        || previous.protected_paths != settings.protected_paths
        || previous.unprotected_paths != settings.unprotected_paths
        || previous.ignored_paths != settings.ignored_paths
}

fn invalidate_ai_context(state: &Shared) {
    for budget in state.budgets.lock().unwrap().values() {
        budget.cancel.store(true, Ordering::SeqCst);
    }
    state.context_previews.lock().unwrap().clear();
    state.sample_previews.lock().unwrap().clear();
}

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
        if let Some(ids) = &settings.history_reference_ids {
            if ids.len() > 100
                || ids.iter().any(|id| id.is_empty() || id.len() > 128)
                || ids.iter().collect::<std::collections::HashSet<_>>().len() != ids.len()
            {
                return Err("历史回收参考最多选择 100 条，记录编号不能重复或为空".into());
            }
        }
        let previous = state.store.settings().map_err(error)?;
        if !settings.llm.base_url.is_empty() {
            cleaner_llm::client::endpoint(&settings.llm.base_url).map_err(error)?;
        }
        cleaner_llm::client::validate_output_limit(settings.llm.max_output_tokens)
            .map_err(error)?;
        settings.language = if settings.language == "en" {
            "en".into()
        } else {
            "zh-CN".into()
        };
        settings.scan_retention = if settings.scan_retention == 0 {
            0
        } else {
            settings.scan_retention.clamp(2, 1000)
        };
        settings.llm.max_requests = settings.llm.max_requests.clamp(1, 100);
        settings.llm.concurrency = settings.llm.concurrency.clamp(1, 2);
        settings.llm.timeout_seconds = settings.llm.timeout_seconds.clamp(5, 300);
        settings.llm.minimum_bytes = settings.llm.minimum_bytes.max(1048576);
        if update_ai_consent(&previous, &mut settings) || key.is_some() {
            invalidate_ai_context(&state);
        }
        crate::settings_store::save(&state.store, &previous, &settings, key.as_deref())
            .map_err(error)?;
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
        if result.is_ok() {
            invalidate_ai_context(&state);
        }
        result
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn authorized() -> Settings {
        let mut settings = Settings::default();
        settings.llm.enabled = true;
        settings.llm.base_url = "https://old.example/v1".into();
        settings.llm.metadata_consent = true;
        settings.llm.history_reference_enabled = true;
        settings
    }

    #[test]
    fn changing_provider_revokes_both_sending_consents() {
        let previous = authorized();
        let mut next = previous.clone();
        next.llm.base_url = "https://new.example/v1".into();
        assert!(update_ai_consent(&previous, &mut next));
        assert!(!next.llm.metadata_consent);
        assert!(!next.llm.history_reference_enabled);
    }

    #[test]
    fn revoking_history_or_excluding_a_path_invalidates_pending_context() {
        let previous = authorized();
        let mut next = previous.clone();
        assert!(!update_ai_consent(&previous, &mut next));
        next.llm.history_reference_enabled = false;
        assert!(update_ai_consent(&previous, &mut next));
        assert!(next.llm.metadata_consent);
        next = previous.clone();
        next.excluded_llm_paths.push("D:\\private".into());
        assert!(update_ai_consent(&previous, &mut next));
        next = previous.clone();
        next.ignored_paths.push("D:\\ignored".into());
        assert!(update_ai_consent(&previous, &mut next));
        next = previous.clone();
        next.unprotected_paths.push("D:\\allowed".into());
        assert!(update_ai_consent(&previous, &mut next));
    }

    #[test]
    fn changing_request_limits_scope_or_format_stops_pending_batches() {
        let previous = authorized();
        let changes: [fn(&mut Settings); 7] = [
            |next| next.llm.max_requests = 1,
            |next| next.llm.max_output_tokens = 8192,
            |next| next.llm.minimum_bytes = 512 * 1024 * 1024,
            |next| next.llm.format = "text".into(),
            |next| next.llm.automatic = true,
            |next| {
                next.labels.insert("D:\\Example".into(), "示例应用".into());
            },
            |next| next.community_enabled = true,
        ];
        for change in changes {
            let mut next = previous.clone();
            change(&mut next);
            assert!(update_ai_consent(&previous, &mut next));
            assert!(next.llm.metadata_consent);
            assert!(next.llm.history_reference_enabled);
        }
    }
}

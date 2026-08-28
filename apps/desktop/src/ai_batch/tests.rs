use super::*;
use cleaner_domain::{DeletionAdvice, FileRecord, HistoryItem, ModelAssessment, Scan, Settings};
use cleaner_engine::store::Store;
use cleaner_llm::{BatchItemReply, BatchReply};

fn fixture(count: usize) -> (tempfile::TempDir, Shared, Vec<AnalysisContext>, Budget) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path().join("batch.sqlite")).unwrap();
    store
        .save_scan(&Scan {
            id: "s".into(),
            root: "D:\\BatchFixture".into(),
            status: "complete".into(),
            ..Default::default()
        })
        .unwrap();
    let files: Vec<_> = (0..count)
        .map(|i| FileRecord {
            path: format!("D:\\BatchFixture\\group\\{i}.bin"),
            parent: "D:\\BatchFixture\\group".into(),
            name: format!("{i}.bin"),
            logical_bytes: 200_000_000,
            file_count: 1,
            complete: true,
            ..Default::default()
        })
        .collect();
    Store::insert_batch(&mut store.connection().unwrap(), "s", &files).unwrap();
    let mut settings = Settings::default();
    settings.llm.enabled = true;
    settings.llm.automatic = true;
    settings.llm.metadata_consent = true;
    settings.llm.base_url = "https://example.test/v1".into();
    settings.llm.model = "test".into();
    store.put("settings", &settings).unwrap();
    let state = crate::state::AppState::new(store);
    let contexts = files
        .iter()
        .map(|file| {
            let file = state.store.by_path("s", &file.path).unwrap();
            ai::snapshot_context(&state, "s", file.id).unwrap()
        })
        .collect();
    let budget = ai::budget(&state, "s", 10);
    (dir, state, contexts, budget)
}

fn assessment(id: i64) -> ModelAssessment {
    ModelAssessment {
        deletion_advice: DeletionAdvice::Review,
        reason: format!("文件 {id} 来源不明，请确认是否仍需保留。"),
        history_matches: vec![],
        evidence: vec!["summary".into()],
    }
}

fn reply(contexts: &[AnalysisContext]) -> BatchReply {
    BatchReply {
        items: contexts
            .iter()
            .rev()
            .map(|context| BatchItemReply {
                entry_id: context.entry_id,
                assessment: Ok(assessment(context.entry_id)),
            })
            .collect(),
        prompt_tokens: Some(321),
        completion_tokens: Some(79),
    }
}

#[test]
fn batch_results_follow_ids_partial_failures_retry_alone_and_usage_is_stored_once() {
    let (_dir, state, contexts, budget) = fixture(3);
    let failed_id = contexts[1].entry_id;
    let outcomes = run_with(
        &state,
        contexts.clone(),
        &budget,
        false,
        |_, received, budget| {
            assert_eq!(received.len(), 3);
            budget.reserve()?;
            let mut result = reply(received);
            result
                .items
                .iter_mut()
                .find(|item| item.entry_id == failed_id)
                .unwrap()
                .assessment = Err("缺少该项建议".into());
            Ok(result)
        },
    )
    .unwrap();
    assert!(matches!(outcomes[0], Ok(AnalysisOutcome::Completed)));
    assert!(matches!(outcomes[1], Ok(AnalysisOutcome::Failed(_))));
    assert!(matches!(outcomes[2], Ok(AnalysisOutcome::Completed)));
    let rows = state
        .store
        .analyses_for_entries(
            "s",
            &contexts.iter().map(|c| c.entry_id).collect::<Vec<_>>(),
        )
        .unwrap();
    assert_eq!(
        rows.iter().filter_map(|row| row.prompt_tokens).sum::<u64>(),
        321
    );
    assert_eq!(
        rows.iter()
            .filter_map(|row| row.completion_tokens)
            .sum::<u64>(),
        79
    );
    assert!(rows
        .iter()
        .all(|row| row.request_id == rows[0].request_id && row.request_item_count == 3));
    for row in rows.iter().filter(|row| row.status == "success") {
        assert_eq!(
            row.assessment.as_ref().unwrap().reason,
            assessment(row.entry_id).reason
        );
    }
    let mut detail = state.store.analyses("s", contexts[2].entry_id).unwrap();
    assert_eq!(detail[0].prompt_tokens, None);
    state.store.hydrate_batch_usage(&mut detail).unwrap();
    assert_eq!(detail[0].prompt_tokens, Some(321));
    assert_eq!(detail[0].completion_tokens, Some(79));
    assert_eq!(
        state.store.analyses("s", contexts[2].entry_id).unwrap()[0].prompt_tokens,
        None
    );
    let outcomes = run_with(&state, contexts, &budget, false, |_, received, budget| {
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].entry_id, failed_id);
        budget.reserve()?;
        Ok(reply(received))
    })
    .unwrap();
    assert!(matches!(outcomes[0], Ok(AnalysisOutcome::Cached)));
    assert!(matches!(outcomes[1], Ok(AnalysisOutcome::Completed)));
    assert_eq!(budget.requests.load(Ordering::SeqCst), 2);
    assert_eq!(state.progress.lock().unwrap().finished, 6);
}

#[test]
fn changed_authorization_marks_inflight_answers_stale_and_stops_future_requests() {
    let (_dir, state, contexts, budget) = fixture(2);
    run_with(
        &state,
        contexts.clone(),
        &budget,
        true,
        |_, received, budget| {
            budget.reserve()?;
            let mut settings = state.store.settings()?;
            settings.llm.metadata_consent = false;
            state.store.put("settings", &settings)?;
            Ok(reply(received))
        },
    )
    .unwrap();
    assert!(contexts
        .iter()
        .all(|context| state.store.analyses("s", context.entry_id).unwrap()[0].status == "stale"));
    assert!(run_with(&state, contexts, &budget, true, |_, _, _| panic!(
        "authorization revoked"
    ))
    .is_err());
    assert_eq!(budget.requests.load(Ordering::SeqCst), 1);
}

#[test]
fn queued_files_are_rechecked_against_current_source_and_size_scope() {
    let (_dir, state, contexts, budget) = fixture(2);
    let mut settings = state.store.settings().unwrap();
    settings.llm.minimum_bytes = 300_000_000;
    state.store.put("settings", &settings).unwrap();
    let outcomes = run_with(&state, contexts.clone(), &budget, false, |_, _, _| {
        panic!("files now below threshold must not be sent")
    })
    .unwrap();
    assert!(outcomes.iter().all(Result::is_err));
    settings.llm.minimum_bytes = 100 * 1024 * 1024;
    settings
        .labels
        .insert("D:\\BatchFixture\\group".into(), "已识别应用".into());
    state.store.put("settings", &settings).unwrap();
    let current: Vec<_> = contexts
        .iter()
        .map(|context| ai::snapshot_context(&state, "s", context.entry_id).unwrap())
        .collect();
    let outcomes = run_with(&state, current, &budget, false, |_, _, _| {
        panic!("files with a newly identified source must not be sent")
    })
    .unwrap();
    assert!(outcomes.iter().all(Result::is_err));
    assert_eq!(budget.requests.load(Ordering::SeqCst), 0);
    assert!(state.store.analysis_entry_ids("s").unwrap().is_empty());
}

#[test]
fn truncated_batches_keep_usage_once_and_exhausted_budget_creates_no_model_failures() {
    let (_dir, state, contexts, mut budget) = fixture(2);
    budget.maximum = 1;
    run_with(&state, contexts.clone(), &budget, false, |_, _, budget| {
        budget.reserve()?;
        Err(client::BatchFailure {
            message: "输出达到长度限制".into(),
            prompt_tokens: Some(100),
            completion_tokens: Some(1800),
        }
        .into())
    })
    .unwrap();
    let before = state.store.analyses("s", contexts[0].entry_id).unwrap();
    assert_eq!(before[0].status, "failed");
    assert_eq!(before[0].completion_tokens, Some(1800));
    assert!(
        run_with(&state, contexts.clone(), &budget, false, |_, _, budget| {
            budget.reserve()?;
            panic!("budget exhausted before sending")
        })
        .unwrap_err()
        .contains("预算")
    );
    assert_eq!(
        state
            .store
            .analyses("s", contexts[0].entry_id)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(state.progress.lock().unwrap().finished, 2);
}

#[test]
fn opening_new_version_discards_old_ai_content_but_preserves_scan_settings_history_and_budget() {
    let (_dir, state, contexts, budget) = fixture(1);
    run_with(
        &state,
        contexts.clone(),
        &budget,
        false,
        |_, received, budget| {
            budget.reserve()?;
            Ok(reply(received))
        },
    )
    .unwrap();
    state
        .store
        .connection()
        .unwrap()
        .execute(
            "INSERT INTO analyses VALUES('old','s',?1,0,?2)",
            (
                contexts[0].entry_id,
                serde_json::json!({"assessment":{"purpose":"旧版长分析"}}).to_string(),
            ),
        )
        .unwrap();
    state
        .store
        .add_history(&HistoryItem {
            id: "history".into(),
            batch_id: "old-batch".into(),
            path: "D:\\old.bin".into(),
            bytes: 100,
            time: 1,
            status: "recycled".into(),
            message: String::new(),
            free_space_delta: 0,
            snapshot: None,
        })
        .unwrap();
    let scan_before = serde_json::to_value(state.store.scan("s").unwrap()).unwrap();
    let settings_before = serde_json::to_value(state.store.settings().unwrap()).unwrap();
    let reopened = Store::open(&state.store.path).unwrap();
    assert_eq!(
        reopened.analyses("s", contexts[0].entry_id).unwrap().len(),
        1
    );
    assert_eq!(
        reopened
            .entry("s", contexts[0].entry_id)
            .unwrap()
            .logical_bytes,
        200_000_000
    );
    assert_eq!(
        serde_json::to_value(reopened.scan("s").unwrap()).unwrap(),
        scan_before
    );
    assert_eq!(
        serde_json::to_value(reopened.settings().unwrap()).unwrap(),
        settings_before
    );
    assert_eq!(reopened.get::<u32>("llm-budget:s").unwrap(), Some(1));
    assert_eq!(reopened.history().unwrap().len(), 1);
}

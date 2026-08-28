use super::*;
use crate::{
    client::{synthetic_context, tests::mock_request, BudgetExhausted},
    MAX_BATCH_METADATA_BYTES,
};
use cleaner_domain::{DeletionAdvice, HistoryReference};
use std::sync::atomic::Ordering;

fn contexts() -> Vec<AnalysisContext> {
    (1..=3)
        .map(|id| AnalysisContext {
            entry_id: id,
            path: format!("%USERPROFILE%/Example/{id}.bin"),
            ..synthetic_context()
        })
        .collect()
}

fn item(id: i64, advice: &str) -> Value {
    json!({"entryId":id,"recommendation":advice,"reason":"请先核实是否仍需使用。","historyMatchIds":[]})
}

fn response(items: Vec<Value>) -> Value {
    json!({
        "choices": [{
            "message": {"role":"assistant","content":json!({"items":items}).to_string()},
            "finish_reason":"stop"
        }],
        "usage": {"prompt_tokens":31,"completion_tokens":19}
    })
}

#[test]
fn batch_uses_one_request_and_one_usage_total_with_independent_item_results() {
    let mut bad = item(2, "review");
    bad["reason"] = json!("字".repeat(121));
    let data = response(vec![item(3, "keep"), bad, item(1, "consider_delete")]);
    let budget = Budget::new(1);
    let (reply, body) = mock_request(LlmSettings::default(), 200, &data.to_string(), |settings| {
        analyze_batch(settings, None, &contexts(), &budget)
    });
    let reply = reply.unwrap();
    assert_eq!(
        (reply.prompt_tokens, reply.completion_tokens),
        (Some(31), Some(19))
    );
    assert_eq!(
        reply
            .items
            .iter()
            .map(|item| item.entry_id)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        reply.items[0].assessment.as_ref().unwrap().deletion_advice,
        DeletionAdvice::ConsiderDelete
    );
    assert!(reply.items[1].assessment.is_err());
    assert_eq!(
        reply.items[2].assessment.as_ref().unwrap().deletion_advice,
        DeletionAdvice::Keep
    );
    assert_eq!(budget.requests.load(Ordering::SeqCst), 1);
    let payload: Value =
        serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(payload["metadata"]["items"].as_array().unwrap().len(), 3);
    assert_eq!(
        payload["metadata"]["directories"].as_array().unwrap().len(),
        1
    );
    assert!(payload.get("authorizedTextSamples").is_none());
    let properties = &body["response_format"]["json_schema"]["schema"]["properties"]["items"]
        ["items"]["properties"];
    assert_eq!(properties.as_object().unwrap().len(), 4);
    assert_eq!(properties["entryId"]["enum"], json!([1, 2, 3]));
}

#[test]
fn valid_partial_output_does_not_retry_or_claim_missing_files_succeeded() {
    let budget = Budget::new(10);
    let data = response(vec![item(2, "review"), item(999, "keep")]);
    let (reply, _) = mock_request(LlmSettings::default(), 200, &data.to_string(), |settings| {
        analyze_batch(settings, None, &contexts(), &budget)
    });
    let reply = reply.unwrap();
    assert!(reply.items[0].assessment.is_err());
    assert!(reply.items[1].assessment.is_ok());
    assert!(reply.items[2].assessment.is_err());
    assert_eq!(budget.requests.load(Ordering::SeqCst), 1);
}

#[test]
fn truncated_and_malformed_results_fail_the_batch_without_retry_and_retain_usage() {
    for truncated in [false, true] {
        let mut data = response(vec![item(1, "review")]);
        if truncated {
            data["choices"][0]["finish_reason"] = json!("length");
        } else {
            data["choices"][0]["message"]["content"] = json!("{\"items\":[");
        }
        let budget = Budget::new(10);
        let (reply, _) = mock_request(LlmSettings::default(), 200, &data.to_string(), |settings| {
            analyze_batch(settings, None, &contexts(), &budget)
        });
        let error = reply.unwrap_err();
        let failure = error.downcast_ref::<BatchFailure>().unwrap();
        assert_eq!(
            (failure.prompt_tokens, failure.completion_tokens),
            (Some(31), Some(19))
        );
        assert_eq!(budget.requests.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn invalid_metadata_or_revoked_history_fails_before_reserving_a_request() {
    let settings = LlmSettings {
        model: "mock".into(),
        ..Default::default()
    };
    let budget = Budget::new(10);
    for count in [0, 21] {
        let inputs: Vec<_> = (0..count)
            .map(|entry_id| AnalysisContext {
                entry_id,
                ..synthetic_context()
            })
            .collect();
        assert!(analyze_batch(&settings, None, &inputs, &budget).is_err());
    }
    let mut large = synthetic_context();
    large.note = "a".repeat(MAX_BATCH_METADATA_BYTES);
    assert!(analyze_batch(&settings, None, &[large], &budget).is_err());
    let mut with_history = synthetic_context();
    with_history.history_references = vec![HistoryReference {
        id: "history:a".into(),
        path: "D:/Example/old.bin".into(),
        bytes: 100,
        recycled_at: 1,
        owner: None,
        category: None,
        match_basis: vec!["名称相似".into()],
    }];
    assert!(analyze_batch(&settings, None, &[with_history], &budget)
        .unwrap_err()
        .to_string()
        .contains("授权已关闭"));
    assert_eq!(budget.requests.load(Ordering::SeqCst), 0);
}

#[test]
fn exhausted_budget_is_typed_and_does_not_mark_a_batch_as_a_model_response_failure() {
    let budget = Budget::new(0);
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let settings = LlmSettings {
        base_url: format!("http://{}/v1", server.server_addr()),
        model: "mock".into(),
        ..Default::default()
    };
    let error = analyze_batch(&settings, None, &contexts(), &budget).unwrap_err();
    assert!(error.is::<BudgetExhausted>());
    assert!(!error.is::<BatchFailure>());
    assert_eq!(budget.requests.load(Ordering::SeqCst), 0);
    assert!(server
        .recv_timeout(std::time::Duration::from_millis(10))
        .unwrap()
        .is_none());
}

#[test]
fn batch_format_fallback_keeps_every_item_and_counts_each_http_attempt() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let settings = LlmSettings {
        base_url: format!("http://{}/v1", server.server_addr()),
        model: "mock".into(),
        ..Default::default()
    };
    let worker = std::thread::spawn(move || {
        let mut first_payload = None;
        for attempt in 0..2 {
            let mut request = server
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap()
                .unwrap();
            let mut raw = String::new();
            request.as_reader().read_to_string(&mut raw).unwrap();
            let body: Value = serde_json::from_str(&raw).unwrap();
            let payload = body["messages"][1]["content"].clone();
            if attempt == 0 {
                first_payload = Some(payload);
                request
                    .respond(
                        tiny_http::Response::from_string("unsupported response_format json_schema")
                            .with_status_code(400),
                    )
                    .unwrap();
            } else {
                assert_eq!(Some(payload), first_payload);
                assert_eq!(body["response_format"]["type"], "json_object");
                request
                    .respond(tiny_http::Response::from_string(
                        response(vec![item(1, "review"), item(2, "keep"), item(3, "review")])
                            .to_string(),
                    ))
                    .unwrap();
            }
        }
    });
    let budget = Budget::new(2);
    let reply = analyze_batch(&settings, None, &contexts(), &budget).unwrap();
    assert!(reply.items.iter().all(|item| item.assessment.is_ok()));
    assert_eq!(budget.requests.load(Ordering::SeqCst), 2);
    worker.join().unwrap();
}

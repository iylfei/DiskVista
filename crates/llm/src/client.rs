use crate::batch::analyze_contexts;
pub use crate::batch::BatchFailure;
pub use crate::transport::endpoint;
use anyhow::{anyhow, bail, Result};
use cleaner_domain::{AnalysisContext, LlmSettings, ModelAssessment, MAX_OUTPUT_TOKEN_LIMIT};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};

pub const PROMPT_VERSION: &str = "cleaner-deletion-v3-batch";

#[derive(Debug, Clone, Copy)]
pub struct BudgetExhausted;

impl std::fmt::Display for BudgetExhausted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("本次扫描的请求预算已用完（兼容性重试也计入）")
    }
}

impl std::error::Error for BudgetExhausted {}

#[derive(Clone)]
pub struct Budget {
    pub requests: Arc<AtomicU32>,
    pub maximum: u32,
    pub cancel: Arc<AtomicBool>,
    pub persist: Option<Arc<dyn Fn(u32) -> Result<()> + Send + Sync>>,
}
impl Budget {
    pub fn new(maximum: u32) -> Self {
        Self {
            requests: Arc::new(AtomicU32::new(0)),
            maximum,
            cancel: Arc::new(AtomicBool::new(false)),
            persist: None,
        }
    }
    pub fn reserve(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            bail!("分析已取消");
        }
        let previous = self
            .requests
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                if n < self.maximum {
                    Some(n + 1)
                } else {
                    None
                }
            })
            .map_err(|_| BudgetExhausted)?;
        if let Some(persist) = &self.persist {
            persist(previous + 1)?;
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct Reply {
    pub assessment: ModelAssessment,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}
pub fn config_hash(settings: &LlmSettings, rule_version: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(format!(
            "{}|{}|{}|{}|{}|{}|{}",
            settings.base_url,
            settings.model,
            settings.format,
            settings.token_parameter,
            rule_version,
            PROMPT_VERSION,
            settings.history_reference_enabled
        ))
    )
}

pub fn validate_output_limit(limit: u32) -> Result<()> {
    if !(1..=MAX_OUTPUT_TOKEN_LIMIT).contains(&limit) {
        bail!("单次输出上限须为 1 到 1048576 之间的整数；实际可用上限由服务商和模型决定");
    }
    Ok(())
}

pub fn analyze(
    settings: &LlmSettings,
    key: Option<&str>,
    context: &AnalysisContext,
    samples: &Value,
    budget: &Budget,
) -> Result<Reply> {
    let reply = analyze_contexts(
        settings,
        key,
        std::slice::from_ref(context),
        Some(samples),
        budget,
    )?;
    let item = reply
        .items
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("AI 未返回该文件的分析结果"))?;
    match item.assessment {
        Ok(assessment) => Ok(Reply {
            assessment,
            prompt_tokens: reply.prompt_tokens,
            completion_tokens: reply.completion_tokens,
        }),
        Err(message) => Err(BatchFailure {
            message,
            prompt_tokens: reply.prompt_tokens,
            completion_tokens: reply.completion_tokens,
        }
        .into()),
    }
}
#[cfg(test)]
pub(crate) fn synthetic_context() -> AnalysisContext {
    AnalysisContext {
        scan_id: "connection-test".into(),
        entry_id: 0,
        fingerprint: "synthetic".into(),
        path: "%USERPROFILE%/Example/Cache".into(),
        logical_bytes: 1048576,
        file_count: 1,
        modified: 0,
        accessed: 0,
        evidence: vec![],
        files: vec![],
        history_references: vec![],
        truncated: false,
        note: "这是合成连接测试，不含用户机器信息。".into(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn url_policy() {
        assert!(endpoint("https://example.com/v1")
            .unwrap()
            .as_str()
            .ends_with("/v1/chat/completions"));
        assert!(endpoint("http://127.0.0.1:1234/v1").is_ok());
        for u in [
            "http://example.com",
            "file:///C:/secrets",
            "https://key@example.com",
            "https://x?a=key",
        ] {
            assert!(endpoint(u).is_err());
        }
    }
    #[test]
    fn hard_budget() {
        let b = Budget::new(2);
        assert!(b.reserve().is_ok());
        assert!(b.reserve().is_ok());
        assert!(b.reserve().unwrap_err().is::<BudgetExhausted>());
        assert_eq!(b.requests.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn history_requires_its_own_authorization_before_any_request() {
        let mut context = synthetic_context();
        context
            .history_references
            .push(cleaner_domain::HistoryReference {
                id: "history:h".into(),
                path: "%USERPROFILE%\\Example\\old.zip".into(),
                bytes: 1024,
                recycled_at: 1,
                owner: None,
                category: None,
                match_basis: vec!["名称特征相似".into()],
            });
        let budget = Budget::new(10);
        let error = analyze(&LlmSettings::default(), None, &context, &json!([]), &budget)
            .err()
            .unwrap();
        assert!(error.to_string().contains("授权已关闭"));
        assert_eq!(budget.requests.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn authorized_history_is_only_metadata_and_has_validatable_evidence_ids() {
        let mut context = synthetic_context();
        context
            .history_references
            .push(cleaner_domain::HistoryReference {
                id: "history:h".into(),
                path: "%USERPROFILE%\\Example\\old.zip".into(),
                bytes: 1024,
                recycled_at: 1,
                owner: None,
                category: None,
                match_basis: vec!["名称特征相似".into()],
            });
        let assessment = json!({"items":[{"entryId":0,"recommendation":"review","reason":"名称相似，仍需确认用途","historyMatchIds":["history:h"]}]});
        let response = json!({"choices":[{"message":{"content":assessment.to_string()},"finish_reason":"stop"}]}).to_string();
        let settings = LlmSettings {
            history_reference_enabled: true,
            ..Default::default()
        };
        let (reply, body) = mock_request(settings, 200, &response, |settings| {
            analyze(settings, None, &context, &json!([]), &Budget::new(1))
        });
        assert_eq!(reply.unwrap().assessment.history_matches.len(), 1);
        let payload: Value =
            serde_json::from_str(body["messages"][1]["content"].as_str().unwrap()).unwrap();
        assert_eq!(
            payload["metadata"]["historyReferences"][0]["id"],
            "history:h"
        );
        assert_eq!(payload["authorizedTextSamples"], json!([]));
        assert!(payload["metadata"]["items"][0]["historyReferences"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reference| reference["id"] == "history:h"));
    }
    #[test]
    fn cancellation() {
        let b = Budget::new(2);
        b.cancel.store(true, Ordering::SeqCst);
        assert!(b.reserve().is_err());
    }
    pub(crate) fn mock_request<T>(
        mut settings: LlmSettings,
        status: u16,
        body: &str,
        run: impl FnOnce(&LlmSettings) -> T,
    ) -> (T, Value) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        settings.base_url = format!("http://{}/v1", server.server_addr());
        settings.model = "mock".into();
        let body = body.to_owned();
        let thread = std::thread::spawn(move || {
            let mut request = server
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .expect("expected one request");
            assert_eq!(request.url(), "/v1/chat/completions");
            let mut payload = String::new();
            request.as_reader().read_to_string(&mut payload).unwrap();
            request
                .respond(tiny_http::Response::from_string(body).with_status_code(status))
                .unwrap();
            serde_json::from_str(&payload).unwrap()
        });
        let result = run(&settings);
        (result, thread.join().unwrap())
    }
    fn single_response(status: u16, body: &str, maximum: u32) -> (Result<Reply>, u32) {
        let budget = Budget::new(maximum);
        let (reply, _) = mock_request(LlmSettings::default(), status, body, |settings| {
            analyze(settings, None, &synthetic_context(), &json!([]), &budget)
        });
        (reply, budget.requests.load(Ordering::SeqCst))
    }

    fn complete_response() -> Value {
        let assessment = json!({"items":[{"entryId":0,"recommendation":"review","reason":"请人工核实用途","historyMatchIds":[]}]});
        json!({"choices":[{"message":{"role":"assistant","content":assessment.to_string()},"finish_reason":"stop"}]})
    }

    #[test]
    fn old_settings_keep_the_previous_output_budget_and_custom_values_roundtrip() {
        let mut settings: LlmSettings =
            serde_json::from_value(json!({"model":"legacy","maxRequests":10})).unwrap();
        assert_eq!(settings.max_output_tokens, 1800);
        settings.max_output_tokens = MAX_OUTPUT_TOKEN_LIMIT;
        let encoded = serde_json::to_value(&settings).unwrap();
        assert_eq!(encoded["maxOutputTokens"], json!(MAX_OUTPUT_TOKEN_LIMIT));
        let restored: LlmSettings = serde_json::from_value(encoded).unwrap();
        assert_eq!(restored.max_output_tokens, MAX_OUTPUT_TOKEN_LIMIT);
        assert_eq!(restored.max_requests, 10);
        assert_eq!(restored.model, "legacy");
        for invalid in [json!(-1), json!(1.5), json!(null)] {
            assert!(
                serde_json::from_value::<LlmSettings>(json!({"maxOutputTokens":invalid})).is_err()
            );
        }
    }

    #[test]
    fn analysis_uses_the_configured_output_budget_and_selected_parameter() {
        for (parameter, limit) in [
            ("max_tokens", 16_384),
            ("max_completion_tokens", 131_072),
            ("max_tokens", MAX_OUTPUT_TOKEN_LIMIT),
            ("none", 65_536),
        ] {
            let settings = LlmSettings {
                token_parameter: parameter.into(),
                max_output_tokens: limit,
                ..Default::default()
            };
            let budget = Budget::new(10);
            let (reply, body) = mock_request(
                settings,
                200,
                &complete_response().to_string(),
                |settings| analyze(settings, None, &synthetic_context(), &json!([]), &budget),
            );
            assert!(reply.is_ok());
            for field in ["max_tokens", "max_completion_tokens"] {
                if parameter == field {
                    assert_eq!(body[field], json!(limit));
                } else {
                    assert!(body.get(field).is_none());
                }
            }
            assert_eq!(budget.requests.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn invalid_output_budgets_fail_before_sending_or_reserving_a_request() {
        for limit in [0, MAX_OUTPUT_TOKEN_LIMIT + 1, u32::MAX] {
            let settings = LlmSettings {
                base_url: "https://example.invalid/v1".into(),
                model: "mock".into(),
                max_output_tokens: limit,
                ..Default::default()
            };
            let budget = Budget::new(10);
            let result = analyze(&settings, None, &synthetic_context(), &json!([]), &budget);
            assert!(result.err().unwrap().to_string().contains("单次输出上限"));
            assert_eq!(budget.requests.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn larger_reasoning_responses_follow_the_output_budget_but_remain_bounded() {
        let mut response = complete_response();
        response["choices"][0]["message"]["reasoning_content"] = json!("分析".repeat(60_000));
        let response = response.to_string();
        assert!(response.len() > 262_144);
        for parameter in ["max_tokens", "max_completion_tokens", "none"] {
            let settings = LlmSettings {
                max_output_tokens: 32_768,
                token_parameter: parameter.into(),
                ..Default::default()
            };
            let (reply, _) = mock_request(settings, 200, &response, |settings| {
                analyze(
                    settings,
                    None,
                    &synthetic_context(),
                    &json!([]),
                    &Budget::new(1),
                )
            });
            assert_eq!(reply.unwrap().assessment.reason, "请人工核实用途");
        }
        let settings = LlmSettings {
            max_output_tokens: 32_768,
            ..Default::default()
        };
        let (reply, _) = mock_request(settings, 200, &"思".repeat(240_000), |settings| {
            analyze(
                settings,
                None,
                &synthetic_context(),
                &json!([]),
                &Budget::new(1),
            )
        });
        assert!(reply.err().unwrap().to_string().contains("大小限制"));
    }

    #[test]
    fn connection_probe_sends_only_a_short_message_and_respects_token_parameter() {
        for parameter in ["max_tokens", "max_completion_tokens", "none"] {
            let settings = LlmSettings {
                token_parameter: parameter.into(),
                max_output_tokens: 16_384,
                ..Default::default()
            };
            let response = json!({
                "choices": [{"message": {"role": "assistant", "content": "OK"}, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 5, "completion_tokens": 1}
            });
            let (reply, body) = mock_request(settings, 200, &response.to_string(), |settings| {
                crate::connection::test_connection(settings, None)
            });
            let reply = reply.unwrap();
            assert!(reply.reply_complete);
            assert_eq!(
                (reply.prompt_tokens, reply.completion_tokens),
                (Some(5), Some(1))
            );
            let mut expected = json!({
                "model": "mock",
                "messages": [{"role": "user", "content": "Reply with OK only."}],
                "stream": false
            });
            if parameter != "none" {
                expected[parameter] = json!(128);
            }
            assert_eq!(body, expected);
        }
    }
    #[test]
    fn truncated_probe_confirms_connectivity_but_truncated_analysis_is_rejected() {
        let response = json!({
            "choices": [{
                "message": {"role": "assistant", "content": null, "reasoning_content": "..."},
                "finish_reason": "length"
            }]
        })
        .to_string();
        let (reply, _) = mock_request(LlmSettings::default(), 200, &response, |settings| {
            crate::connection::test_connection(settings, None)
        });
        assert!(!reply.unwrap().reply_complete);
        let (result, count) = single_response(200, &response, 10);
        let message = result.err().unwrap().to_string();
        assert!(message.contains("长度限制") && message.contains("1800 token"));
        assert_eq!(count, 1);
        let settings = LlmSettings {
            token_parameter: "none".into(),
            ..Default::default()
        };
        let (result, _) = mock_request(settings, 200, &response, |settings| {
            analyze(
                settings,
                None,
                &synthetic_context(),
                &json!([]),
                &Budget::new(1),
            )
        });
        assert!(result.err().unwrap().to_string().contains("未发送长度参数"));
    }
    #[test]
    fn connection_probe_rejects_http_errors_and_invalid_model_responses() {
        for (status, body, expected) in [
            (401, "private response body", "密钥"),
            (429, "private response body", "限流"),
            (400, "unsupported response_format", "HTTP 400"),
            (200, "not JSON", "不是 JSON"),
            (200, "{}", "有效的模型响应"),
            (
                200,
                r#"{"choices":[{"message":{},"finish_reason":"length"}]}"#,
                "有效的模型响应",
            ),
            (
                200,
                r#"{"choices":[{"message":{"role":"assistant","content":""},"finish_reason":"stop"}]}"#,
                "空的测试回复",
            ),
            (
                200,
                r#"{"choices":[{"message":{"role":"assistant","refusal":"declined"},"finish_reason":"stop"}]}"#,
                "拒绝",
            ),
        ] {
            let (result, _) = mock_request(LlmSettings::default(), status, body, |settings| {
                crate::connection::test_connection(settings, None)
            });
            let error = result.err().expect("probe should fail").to_string();
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains("private response body"));
        }
    }
    #[test]
    fn invalid_json_refusal_and_rate_limit_are_isolated() {
        for (status, body) in [
            (429, "limited"),
            (200, "not JSON"),
            (200, r#"{"choices":[{"message":{"refusal":"declined"}}]}"#),
        ] {
            let (result, count) = single_response(status, body, 10);
            assert!(result.is_err());
            assert_eq!(count, 1);
        }
    }
    #[test]
    fn compatibility_fallback_cannot_exceed_budget() {
        let (result, count) = single_response(400, "unsupported response_format", 1);
        assert!(result.err().unwrap().is::<BudgetExhausted>());
        assert_eq!(count, 1);
    }
    #[test]
    fn concurrent_requests_respect_the_exact_budget() {
        let budget = Budget::new(10);
        std::thread::scope(|scope| {
            for _ in 0..40 {
                let b = &budget;
                scope.spawn(move || {
                    let _ = b.reserve();
                });
            }
        });
        assert_eq!(budget.requests.load(Ordering::SeqCst), 10);
    }
    #[test]
    fn timed_out_request_does_not_retry() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let settings = LlmSettings {
            base_url: format!("http://{}/v1", server.server_addr()),
            model: "mock".into(),
            timeout_seconds: 5,
            ..Default::default()
        };
        let thread = std::thread::spawn(move || {
            let request = server.recv().unwrap();
            std::thread::sleep(Duration::from_secs(6));
            let _ = request.respond(tiny_http::Response::empty(200));
        });
        let budget = Budget::new(10);
        let result = analyze(&settings, None, &synthetic_context(), &json!([]), &budget);
        assert!(result.err().unwrap().to_string().contains("超时"));
        assert_eq!(budget.requests.load(Ordering::SeqCst), 1);
        thread.join().unwrap();
    }
    #[test]
    fn mock_fallback_counts_every_request() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", server.server_addr());
        let thread = std::thread::spawn(move || {
            for i in 0..2 {
                let mut request = server.recv().unwrap();
                let mut body = String::new();
                request.as_reader().read_to_string(&mut body).unwrap();
                assert!(!body.contains("tools\""));
                let value: Value = serde_json::from_str(&body).unwrap();
                if i == 0 {
                    assert_eq!(value["response_format"]["type"], "json_schema");
                    request
                        .respond(
                            tiny_http::Response::from_string(
                                "unsupported response_format json_schema",
                            )
                            .with_status_code(400),
                        )
                        .unwrap();
                } else {
                    assert_eq!(value["response_format"]["type"], "json_object");
                    let data = json!({"items":[{"entryId":0,"recommendation":"review","reason":"请人工核实用途","historyMatchIds":[]}]});
                    request.respond(tiny_http::Response::from_string(json!({"choices":[{"message":{"content":data.to_string()},"finish_reason":"stop"}],"usage":{"prompt_tokens":12,"completion_tokens":13}}).to_string())).unwrap();
                }
            }
        });
        let settings = LlmSettings {
            base_url: base,
            model: "mock".into(),
            ..Default::default()
        };
        let b = Budget::new(2);
        let reply = analyze(&settings, None, &synthetic_context(), &json!([]), &b).unwrap();
        assert_eq!(reply.prompt_tokens, Some(12));
        assert_eq!(b.requests.load(Ordering::SeqCst), 2);
        thread.join().unwrap();
    }
}

use crate::{
    client::Budget,
    transport::{set_output_limit, ChatClient},
};
use anyhow::{anyhow, bail, Result};
use cleaner_domain::LlmSettings;
use serde_json::json;

pub struct ConnectionReply {
    pub reply_complete: bool,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

pub fn test_connection(settings: &LlmSettings, key: Option<&str>) -> Result<ConnectionReply> {
    let client = ChatClient::new(settings)?;
    let mut body = json!({
        "model": settings.model,
        "messages": [{"role": "user", "content": "Reply with OK only."}],
        "stream": false
    });
    set_output_limit(&mut body, settings, 128);
    let envelope = client.send(&body, key, &Budget::new(1))?.into_envelope()?;
    let choice = &envelope["choices"][0];
    let message = choice["message"]
        .as_object()
        .ok_or_else(|| anyhow!("服务未返回有效的模型响应"))?;
    if message.get("role").and_then(|v| v.as_str()) != Some("assistant") {
        bail!("服务未返回有效的模型响应");
    }
    if message.get("refusal").is_some_and(|v| !v.is_null()) {
        bail!("模型拒绝了测试请求");
    }
    let reply_complete = match choice["finish_reason"].as_str() {
        Some("stop") => {
            if !message
                .get("content")
                .and_then(|v| v.as_str())
                .is_some_and(|text| !text.trim().is_empty())
            {
                bail!("服务返回了空的测试回复");
            }
            true
        }
        Some("length") => false,
        Some("content_filter") => bail!("测试请求被服务过滤"),
        _ => bail!("服务未返回有效的模型响应"),
    };
    Ok(ConnectionReply {
        reply_complete,
        prompt_tokens: envelope["usage"]["prompt_tokens"].as_u64(),
        completion_tokens: envelope["usage"]["completion_tokens"].as_u64(),
    })
}

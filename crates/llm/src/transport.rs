use crate::client::Budget;
use anyhow::{anyhow, bail, Result};
use cleaner_domain::LlmSettings;
use reqwest::{blocking::Client, StatusCode, Url};
use serde_json::{json, Value};
use std::{io::Read, sync::atomic::Ordering, time::Duration};

pub fn endpoint(base: &str) -> Result<Url> {
    let mut url = Url::parse(base.trim()).map_err(|_| anyhow!("API 地址格式无效"))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("API 地址不能包含账号、密码、查询参数或片段");
    }
    let local = url
        .host_str()
        .is_some_and(|h| h == "localhost" || h == "127.0.0.1" || h == "[::1]" || h == "::1");
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        bail!("云端 API 必须使用 HTTPS；HTTP 仅允许本机回环地址");
    }
    let path = url.path().trim_end_matches('/');
    if !path.ends_with("/chat/completions") {
        let p = format!("{path}/chat/completions");
        url.set_path(&p);
    }
    Ok(url)
}

pub(crate) fn set_output_limit(body: &mut Value, settings: &LlmSettings, limit: u32) {
    match settings.token_parameter.as_str() {
        "max_completion_tokens" => body["max_completion_tokens"] = json!(limit),
        "none" => {}
        _ => body["max_tokens"] = json!(limit),
    }
}

pub(crate) struct ChatClient {
    client: Client,
    url: Url,
}

impl ChatClient {
    pub fn new(settings: &LlmSettings) -> Result<Self> {
        let url = endpoint(&settings.base_url)?;
        if settings.model.trim().is_empty() {
            bail!("请填写模型 ID");
        }
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(settings.timeout_seconds.clamp(5, 300)))
            .connect_timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self { client, url })
    }

    pub fn send(&self, body: &Value, key: Option<&str>, budget: &Budget) -> Result<ChatResponse> {
        budget.reserve()?;
        let mut request = self.client.post(self.url.clone()).json(body);
        if let Some(key) = key.filter(|k| !k.is_empty()) {
            request = request.bearer_auth(key);
        }
        let response = request.send().map_err(|e| {
            anyhow!(if e.is_timeout() {
                "AI 请求超时"
            } else {
                "AI 网络连接失败，请检查服务地址和网络"
            })
        })?;
        let status = response.status();
        let mut raw = String::new();
        response
            .take(262145)
            .read_to_string(&mut raw)
            .map_err(|_| anyhow!("服务返回了无效 UTF-8 响应"))?;
        if raw.len() > 262144 {
            bail!("服务响应超过限制");
        }
        if budget.cancel.load(Ordering::Relaxed) {
            bail!("分析已取消，响应已丢弃");
        }
        Ok(ChatResponse { status, raw })
    }
}

pub(crate) struct ChatResponse {
    status: StatusCode,
    raw: String,
}

impl ChatResponse {
    pub fn unsupported_format(&self) -> bool {
        let lower = self.raw.to_lowercase();
        (self.status.as_u16() == 400 || self.status.as_u16() == 422)
            && (lower.contains("response_format") || lower.contains("json_schema"))
            && (lower.contains("unsupported")
                || lower.contains("not support")
                || lower.contains("unknown"))
    }

    pub fn into_envelope(self) -> Result<Value> {
        if !self.status.is_success() {
            bail!(
                "API 返回 HTTP {}{}",
                self.status.as_u16(),
                match self.status.as_u16() {
                    401 | 403 => "；请检查 API 密钥和模型访问权限",
                    429 => "（限流；未自动重试）",
                    _ => "；请检查兼容设置，不记录服务响应正文",
                }
            );
        }
        serde_json::from_str(&self.raw).map_err(|_| anyhow!("服务返回的响应不是 JSON"))
    }
}

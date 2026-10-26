use async_openai::{Client, config::OpenAIConfig};
use serde_json::{Value, json};
use std::time::Duration;

use super::AssistantError;

/// Deployment configuration, never supplied by a conversation.
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    /// Additional protocol options; cannot override model, messages, tools or streaming.
    pub request_options: Value,
}

pub struct Llm {
    client: Client<OpenAIConfig>,
    model: String,
    options: Value,
}

impl Llm {
    pub fn new(config: LlmConfig) -> Result<Self, String> {
        let url = reqwest::Url::parse(&config.base_url).map_err(|_| "Invalid LLM_BASE_URL")?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || config.api_key.trim().is_empty()
            || config.model.trim().is_empty()
        {
            return Err("Invalid LLM configuration".into());
        }
        let options = config
            .request_options
            .as_object()
            .ok_or("LLM_REQUEST_OPTIONS must be an object")?;
        if ["messages", "model", "tools", "stream", "n", "tool_choice"]
            .iter()
            .any(|key| options.contains_key(*key))
        {
            return Err("LLM_REQUEST_OPTIONS contains a reserved field".into());
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Cannot build LLM client")?;
        Ok(Self {
            client: Client::build(
                client.clone(),
                OpenAIConfig::new()
                    .with_org_id("")
                    .with_project_id("")
                    .with_api_base(config.base_url.trim_end_matches('/'))
                    .with_api_key(config.api_key),
            )
            .with_http_service(async_openai::middleware::ReqwestService::new(client)),
            model: config.model,
            options: config.request_options,
        })
    }

    pub(super) async fn complete(
        &self,
        messages: &[Value],
        tools: Value,
    ) -> Result<Value, AssistantError> {
        let started = std::time::Instant::now();
        tracing::debug!("开始LLM请求");
        let mut request = self.options.clone();
        request.as_object_mut().expect("validated options").extend(
            json!({"model": self.model, "messages": messages, "tools": tools, "stream": false, "n": 1})
                .as_object().expect("object").clone()
        );
        // BYOT preserves protocol extension fields in assistant messages and history.
        let response: Value = self
            .client
            .chat()
            .create_byot(request)
            .await
            .map_err(|error| {
                // 不打印SDK错误原文，其中可能包含URL、响应正文或内部提示词。
                let elapsed_ms = started.elapsed().as_millis() as u64;
                let (category, status) = match &error {
                    async_openai::error::OpenAIError::Reqwest(error) => (
                        if error.is_timeout() {
                            "timeout"
                        } else if error.is_connect() {
                            "connection"
                        } else {
                            "transport"
                        },
                        error.status().map(|s| s.as_u16()),
                    ),
                    async_openai::error::OpenAIError::ApiError(error) => {
                        ("upstream", Some(error.status_code.as_u16()))
                    }
                    async_openai::error::OpenAIError::JSONDeserialize(..) => ("protocol", None),
                    _ => ("client", None),
                };
                tracing::warn!(elapsed_ms, category, status, "LLM请求失败");
                if category == "timeout" {
                    AssistantError::RequestTimeout
                } else {
                    AssistantError::ModelUnavailable
                }
            })?;
        let choice = &response["choices"][0];
        if !matches!(
            choice["finish_reason"].as_str(),
            Some("stop" | "tool_calls")
        ) {
            tracing::warn!(category = "finish_reason", "LLM响应协议异常");
            return Err(AssistantError::ModelUnavailable);
        }
        let message = &choice["message"];
        if message["role"] != "assistant" {
            tracing::warn!(category = "message_role", "LLM响应协议异常");
            return Err(AssistantError::ModelUnavailable);
        }
        tracing::debug!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "LLM请求完成"
        );
        Ok(message.clone())
    }
}

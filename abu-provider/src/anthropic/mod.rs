#[allow(unused)]
mod dto;
use dto::*;
use reqwest::Client;
use abu_base::{chat::{AssistantMessage, ChatRequest, ChatResponse, FinishReason, ToolCall}, common::Usage};
use crate::{ProvideError, ProvideResult};
use super::ChatProvide;

const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone)]
pub struct Anthropic {
    client: Client,
    base_url: String,
    api_key: String,
}

impl Anthropic {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: ANTHROPIC_BASE_URL.to_string(),
            api_key: api_key.into()
        }
    }

    pub fn from_env() -> ProvideResult<Self> {
        let base_url = std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| ANTHROPIC_BASE_URL.to_string());
        let api_key = std::env::var("ANTHROPIC_API_KEY")?;
        Ok(Self { client: Client::new(), base_url, api_key })
    }

    async fn send_request<Req, Res>(&self, endpoint: &str, body: &Req) -> ProvideResult<Res>
    where
        Req: serde::Serialize,
        Res: serde::de::DeserializeOwned,
    {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint);

        let resp = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| ProvideError::Network(e.to_string()))?;

            if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(ProvideError::Api(format!("Status: {}, Body: {}", status, text)));
        }

        Ok(resp.json::<Res>().await?)
    }
}

#[async_trait::async_trait]
impl ChatProvide for Anthropic {
    type Error = ProvideError;
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, ProvideError> {
        let anthropic_request = AnthropicChatRequestDTO::from_request(request);
        let response: AnthropicResponseDTO = self.send_request("messages", &anthropic_request).await?;
        
        let mut final_content = String::new();
        let mut tool_calls = Vec::new();

        for block in response.content {
            match block {
                AnthropicResponseContentBlockDTO::Text { text } => {
                    final_content.push_str(&text);
                }
                AnthropicResponseContentBlockDTO::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments: input,
                    });
                }
            }
        }

        let finish_reason = match response.stop_reason.as_deref() {
            Some("end_turn") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("tool_use") => FinishReason::ToolCalls,
            _ => FinishReason::Stop,
        };

        Ok(ChatResponse {
            message: AssistantMessage {
                content: final_content,
                tool_calls,
                name: None,
            },
            finish_reason,
            usage: Usage {
                prompt_tokens: response.usage.input_tokens,
                completion_tokens: response.usage.output_tokens,
                total_tokens: response.usage.input_tokens + response.usage.output_tokens,
            },
        })
    }
}
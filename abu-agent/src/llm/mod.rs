use abu_api::{chat::{AssistantMessage, ChatMessage, ChatRequestRef, ChatRequestRefBuilder, ChatResponse, ToolDefinition}, ApiRequest, Credentials};
use crate::{AgentError, AgentResult};

#[derive(Clone)]
pub struct LLM {
    pub credentials: Credentials,
    pub model: String,
}

impl LLM {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            credentials: Credentials::new(base_url, api_key),
            model: model.into(),
        }
    }

    pub fn from_env() -> AgentResult<Self> {
        dotenv::dotenv()?;
        let base_url = std::env::var("BASE_URL")?;
        let api_key = std::env::var("OPENAI_API_KEY")?;
        let model = std::env::var("MODEL_ID")?;

        Ok(Self { credentials: Credentials::new(base_url, api_key), model })
    }

    pub async fn chat(&self, messages: Vec<&ChatMessage>, tools: &[ToolDefinition], temperature: f64) -> AgentResult<AssistantMessage> {
        let request = self.build_message(messages, tools, temperature)?;
        let response = request.send(&self.credentials).await?;
        Self::collect_message(response)
    }

    fn build_message<'a>(&'a self, messages: Vec<&'a ChatMessage>, tools: &'a [ToolDefinition], temperature: f64) -> AgentResult<ChatRequestRef<'a>> {
        let request = ChatRequestRefBuilder::default()
            .model(&self.model)
            .messages(messages)
            .tools(tools)
            .temperature(temperature)
            .build().expect("build req");
        Ok(request)
    }

    fn collect_message(response: ChatResponse) -> AgentResult<AssistantMessage> {
        response
            .choices
            .into_iter()
            .next()
            .ok_or(AgentError::NoChoise)
            .map(|c| c.message)
    }
}

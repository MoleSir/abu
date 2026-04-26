#[allow(unused)]
pub(crate) mod dto;
use dto::*;
use crate::{ProvideError, ProvideResult};
use abu_base::{
    chat::{ChatRequest, ChatResponse}, 
    embed::{EmbedRequest, EmbedResponse},
};
use reqwest::Client;
use super::{ChatProvide, EmbedProvide};

const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Clone)]
pub struct OpenAi {
    pub(crate) client: Client,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
}

impl OpenAi {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: OPENAI_BASE_URL.to_string(),
            api_key: api_key.into()
        }
    }

    pub(crate) fn new_with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into()
        }
    }

    pub fn from_env() -> ProvideResult<Self> {
        let base_url = std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| OPENAI_BASE_URL.to_string());
        let api_key = std::env::var("OPENAI_API_KEY")?;
        Ok(Self { client: Client::new(), base_url, api_key })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    async fn send_request<Req, Res>(&self, endpoint: &str, body: &Req) -> ProvideResult<Res>
    where
        Req: serde::Serialize,
        Res: serde::de::DeserializeOwned,
    {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), endpoint);

        let resp = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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

        let resp = resp.json::<Res>().await?;
        Ok(resp)
    }
}

#[async_trait::async_trait]
impl ChatProvide for OpenAi {
    type Error = ProvideError;
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, ProvideError> {
        let openai_request = OpenAiChatRequestDTO::from_request(request);
        let openai_response: OpenAiChatResponseDTO = self.send_request("chat/completions", &openai_request).await?;
        openai_response.to_chat_response()
    }
}

#[async_trait::async_trait]
impl EmbedProvide for OpenAi {
    type Error = ProvideError;
    async fn embed(&self, request: &EmbedRequest) -> Result<EmbedResponse, ProvideError> {
        let openai_response: OpenAiEmbedResponseDTO = self.send_request("embeddings", &request).await?;
        let embeddings = openai_response.data.into_iter().map(|d| d.embedding).collect();

        Ok(EmbedResponse {
            embeddings,
            usage: openai_response.usage,
        })
    }
}

#[cfg(test)]
mod test {
    use abu_base::{chat::{ChatMessage, ChatRequestBuilder, ToolDefinition}, embed::EmbedRequest};
    use serde_json::json;

    use crate::{openai::OpenAi, ChatProvide, EmbedProvide};

    #[tokio::test]
    async fn test_simple_chat() {
        dotenv::from_filename("./env/openai.env").unwrap();
        let openai = OpenAi::from_env().expect("new client");
        let request = ChatRequestBuilder::default()
            .model(std::env::var("MODEL_ID").expect("No MODEL_ID"))
            .messages([
                ChatMessage::user("hi!"),
            ])
            .build()
            .expect("build request");
                
        let response = openai.chat(&request).await.expect("chat");
        println!("{:#?}", response);
    }

    #[tokio::test]
    async fn test_chat_with_tool() {
        dotenv::from_filename("./env/openai.env").unwrap();
        let openai = OpenAi::from_env().expect("new client");
        let schema = json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The text to echo"
                }
            },
            "required": ["content"],
        });
        let echo_tool = ToolDefinition::new("echo", "echo something", schema);
        let request = ChatRequestBuilder::default()
            .model(std::env::var("MODEL_ID").expect("No MODEL_ID"))
            .messages([
                ChatMessage::user("请调用 echo 工具打印一些东西!"),
            ])
            .tools(vec![echo_tool])
            .build()
            .expect("build request");
                
        let response = openai.chat(&request).await.expect("chat");
        println!("{:#?}", response);
    }

    #[tokio::test]
    async fn test_embed() {
        dotenv::from_filename("./env/embed.env").unwrap();
        let openai = OpenAi::from_env().expect("new client");
        let model = std::env::var("EMBED_MODEL_ID").expect("No EMBED_MODEL_ID");
        let request = EmbedRequest::single("I am yc", model);
        let response = openai.embed(&request).await.expect("chat");
        println!("{:#?}", response);
    }
}
use abu_base::chat::{ChatRequest, ChatResponse};
use crate::{ProvideError, ProvideResult};
use super::{openai::OpenAi, ChatProvide};

const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com";

#[derive(Clone)]
pub struct DeepSeek(OpenAi);

impl DeepSeek {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self(OpenAi::new_with_base_url(api_key, DEEPSEEK_BASE_URL))
    }

    pub fn from_env() -> ProvideResult<Self> {
        let base_url = std::env::var("DEEPSEEK_BASE_URL").unwrap_or_else(|_| DEEPSEEK_BASE_URL.to_string());
        let api_key = std::env::var("DEEPSEEK_API_KEY")?;
        Ok(Self(OpenAi::new_with_base_url(api_key, base_url)))
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.0.base_url = base_url.into();
        self
    }
}

impl ChatProvide for DeepSeek {
    type Error = ProvideError;
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, ProvideError> {
        self.0.chat(request).await
    }
}

#[cfg(test)]
mod test {
    use abu_base::chat::{ChatMessage, ChatRequestBuilder, ToolDefinition};
    use crate::{deepseek::DeepSeek, ChatProvide};

    #[tokio::test]
    async fn test_simple_chat() {
        dotenv::from_filename("./env/deepseek.env").unwrap();
        let deekseep = DeepSeek::from_env().expect("new client");
        let request = ChatRequestBuilder::default()
            .model(std::env::var("MODEL_ID").expect("No MODEL_ID"))
            .messages([
                ChatMessage::user("hi!"),
            ])
            .build()
            .expect("build request");
                
        let response = deekseep.chat(&request).await.expect("chat");
        println!("{:#?}", response);
    }

    #[tokio::test]
    async fn test_chat_with_tool() {
        dotenv::from_filename("./env/deepseek.env").unwrap();
        let deekseep = DeepSeek::from_env().expect("new client");
        let schema = serde_json::json!({
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
                
        let response = deekseep.chat(&request).await.expect("chat");
        println!("{:#?}", response);
    }
}
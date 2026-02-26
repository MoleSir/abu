use std::{collections::HashMap, sync::Arc};
use abu_api::{chat::{AssistantMessage, ChatMessage, ChatRequest, ChatRequestBuilder, ChatResponse, ToolCall, ToolDefinition}, Client};
use crate::{error::{CoreError, CoreResult}, tool::Tool};

#[derive(Clone)]
pub struct LLM {
    client: Arc<Client>,
    model: String,
}

impl LLM {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Arc::new(Client::new(base_url, api_key)),
            model: model.into(),
        }
    }

    pub fn from_env() -> CoreResult<Self> {
        dotenv::dotenv()?;
        let base_url = std::env::var("BASE_URL")?;
        let api_key = std::env::var("OPENAI_API_KEY")?;
        let model = std::env::var("MODEL_ID")?;

        Ok(Self { client: Arc::new(Client::new(base_url, api_key)), model })
    }

    pub fn invoke_sync(&self, messages: impl Into<Vec<ChatMessage>>, tools: impl Into<Vec<ToolDefinition>>) -> CoreResult<AssistantMessage> {
        let request = self.build_message(messages, tools)?;
        let response = self.client.chat_sync(&request)?;
        Self::collect_message(response)
    }

    pub async fn invoke(&self, messages: impl Into<Vec<ChatMessage>>, tools: impl Into<Vec<ToolDefinition>>) -> CoreResult<AssistantMessage> {
        let request = self.build_message(messages, tools)?;
        let response = self.client.chat(&request).await?;
        Self::collect_message(response)
    }

    fn build_message(&self, messages: impl Into<Vec<ChatMessage>>, tools: impl Into<Vec<ToolDefinition>>) -> CoreResult<ChatRequest> {
        let request = ChatRequestBuilder::default()
            .model(&self.model)
            .messages(messages)
            .tools(tools)
            .build()?;
        Ok(request)
    }

    fn collect_message(response: ChatResponse) -> CoreResult<AssistantMessage> {
        response
            .choices
            .into_iter()
            .next()
            .ok_or(CoreError::NoChoise)
            .map(|c| c.message)
    }
}

pub struct LLMSession {
    pub llm: LLM,
    pub tools: HashMap<&'static str, Box<dyn Tool>>,
    pub request: ChatRequest,
}

impl LLMSession {
    pub fn new(llm: LLM) -> Self {
        let request = ChatRequestBuilder::default().model(&llm.model).build().expect("new request");
        Self { llm, request, tools: HashMap::new() }
    }

    pub fn add_message(&mut self, message: impl Into<ChatMessage>) {
        self.request.messages.push(message.into());
    }

    pub fn add_system_message(&mut self, content: impl Into<String>) {
        self.request.messages.push(ChatMessage::system(content));
    }

    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.request.messages.push(ChatMessage::user(content));
    }

    pub fn add_assistant_message(&mut self, message: AssistantMessage) {
        self.request.messages.push(message.into());
    }

    pub fn add_tool_message(&mut self, content: impl Into<String>, tool_call_id: impl Into<String>) {
        self.request.messages.push(ChatMessage::tool(content, tool_call_id));
    }

    pub fn get_tool(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn add_tool<T: Tool + 'static>(&mut self, tool: T) {
        let tool = Box::new(tool);
        self.add_tool_box(tool);
    }

    pub fn add_tool_box(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name();
        self.request.tools.push(tool.to_function_define());
        self.tools.insert(name, tool);
    }

    pub async fn execute_tool(&self, name: &str, arguments: serde_json::Value) -> CoreResult<String> {
        let tool = self.get_tool(name).ok_or_else(|| CoreError::UnsupportTool(name.to_string()))?;
        let value = tool.execute(arguments).await?;
        Ok(value)
    }

    pub async fn execute_toolcall(&self, tool_call: &ToolCall) -> CoreResult<String> {
        let functioncall = &tool_call.function;
        let arguments: serde_json::Value = serde_json::from_str(&functioncall.arguments)?;
        self.execute_tool(&functioncall.name, arguments).await
    }

    pub async fn invoke(&mut self) -> CoreResult<AssistantMessage> {
        let response = self.llm.client.chat(&self.request).await?;
        let message = LLM::collect_message(response)?;
        self.add_message(message.clone());
        Ok(message)
    }

    pub fn invoke_sync(&mut self) -> CoreResult<AssistantMessage> {
        let response = self.llm.client.chat_sync(&self.request)?;
        let message = LLM::collect_message(response)?;
        self.add_message(message.clone());
        Ok(message)
    }
}

#[cfg(test)]
mod test {
    use abu_api::chat::{ChatMessage, FunctionInfo, ToolDefinition, ToolType};
    use crate::llm::LLM;
    use super::LLMSession;

    #[test]
    fn test_llm_chat() {
        let llm = LLM::from_env().expect("new llm");
        let messages = [
            ChatMessage::user("hi"),    
        ];
        let msg = llm.invoke_sync(messages, []).expect("invoke");
        println!("{:#?}", msg);
    }

    #[test]
    fn test_llm_fake_tool() {
        let llm = LLM::from_env().expect("new llm");
        let messages = [
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("请调用 echo，传入你的名字"),    
        ];
        let tools = [
            ToolDefinition {
                r#type: ToolType::Function,
                function: FunctionInfo {
                    name: "echo".to_string(),
                    description: "A echo tool".to_string(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "content": {"type": "string"}
                        },
                        "required": ["content"]
                    })
                }
            }
        ];
        let msg = llm.invoke_sync(messages, tools).expect("invoke");
        println!("{:#?}", msg);
    }

    #[abu_macros::tool(
        struct_name = Echo,
        name = "echo",
        description = "A echo tool.",
        args = [
            "content" : {
                "type": "string",
                "description": "to echo",
            }
        ]
    )]
    fn echo(content: &str) {
        eprintln!("{}", content);
    }

    #[tokio::test]
    async fn test_session() {
        let llm = LLM::from_env().expect("new llm");
        let mut session = LLMSession::new(llm);
        session.add_tool(Echo::new());
        session.add_system_message("You are a helpful assistant.");
        session.add_user_message("请调用 echo，传入你的名字");

        let msg = session.invoke().await.expect("invoke");

        for tool_call in msg.tool_calls.iter() {
            session.execute_toolcall(tool_call).await.expect("tool call");
        }
    }
}
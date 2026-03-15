use std::cell::RefCell;
use abu_base::chat::{ChatMessage, ChatRequest, ChatRequestBuilder, ChatResponse, UserMessage};
use abu_provider::{anthropic::Anthropic, deepseek::DeepSeek, openai::OpenAi, ChatProvide, ProvideError};
use abu_tool::{Tool, ToolDefinition};

pub struct ChatModel<P> {
    request: RefCell<ChatRequest>,
    config: ChatConfig,
    provider: P,
}

#[derive(Default)]
pub struct ChatConfig {
    pub temperature: Option<f64>,
}

impl ChatModel<OpenAi> {
    /// load `OPENAI_BASE_URL` and `OPENAI_API_KEY` in env
    pub fn openai(model: impl Into<String>) -> Result<Self, ChatModelError> {
        let openai = OpenAi::from_env()
            .map_err(|e| ChatModelError::BuildOpenAi(e))?;
        Ok(Self::new(openai, model))
    }
}

impl ChatModel<DeepSeek> {
    /// load `DEEPSEEK_BASE_URL` and `DEEPSEEK_API_KEY` in env
    pub fn deepseek(model: impl Into<String>) -> Result<Self, ChatModelError> {
        let deepseek = DeepSeek::from_env()
            .map_err(|e| ChatModelError::BuildDeepSeek(e))?;
        Ok(Self::new(deepseek, model))

    }
}

impl ChatModel<Anthropic> {
    /// load `ANTHROPIC_BASE_URL` and `ANTHROPIC_API_KEY` in env
    pub fn anthropic(model: impl Into<String>) -> Result<Self, ChatModelError> {
        let anthropic = Anthropic::from_env()
            .map_err(|e| ChatModelError::BuildAnthropic(e))?;
        Ok(Self::new(anthropic, model))
    }
}

impl<P: ChatProvide> ChatModel<P> {
    pub fn new(provider: P, model: impl Into<String>) -> Self {
        let request = ChatRequestBuilder::default()
            .model(model)
            .build()
            .expect("request just need model to build!");
        Self {
            request: RefCell::new(request),
            config: ChatConfig::default(),
            provider
        }
    }

    pub fn set_config(&mut self, config: ChatConfig) {
        self.config = config;
    }

    pub fn bind_tools<'a>(&'a mut self, tools: impl IntoIterator<Item = &'a Box<dyn Tool>>) {
        let tool_defines: Vec<_> = tools.into_iter()
            .map(|t| t.to_function_define())
            .collect(); 
        self.request.borrow_mut().tools = tool_defines;  
    }

    pub fn bind_tool_defines(&mut self, tools: impl Into<Vec<ToolDefinition>>) {
        self.request.borrow_mut().tools = tools.into(); 
    }

    #[inline]
    pub async fn chat(&self, messages: impl IntoChatMessages) -> Result<ChatResponse, ChatModelError> {
        self.send(messages, &self.config, true).await
    }

    #[inline]
    pub async fn chat_no_tools(&self, messages: impl IntoChatMessages) -> Result<ChatResponse, ChatModelError> {
        self.send(messages, &self.config, false).await
    }

    async fn send(&self, messages: impl IntoChatMessages, config: &ChatConfig, with_tools: bool) -> Result<ChatResponse, ChatModelError> {
        // set messages
        let messages = messages.into_messages();
        let mut request = self.request.borrow_mut();
        request.messages = messages;
        // set config
        request.temperature = config.temperature;
        // swap tools
        let mut tools = vec![];
        if !with_tools {
            std::mem::swap(&mut tools, &mut request.tools); 
        }

        // send with provider
        let response = self.provider
            .chat(&request).await
            .map_err(|e| ChatModelError::Provide(Box::new(e)))?;

        // clear messages
        request.messages.clear();
        // reset config
        request.temperature = self.config.temperature;
        // recover tools
        if !with_tools {
            std::mem::swap(&mut tools, &mut request.tools); 
        }

        Ok(response)
    
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ChatModelError {
    #[error("provide error: {0}")]
    Provide(Box<dyn std::error::Error + 'static + Send + Sync>),

    #[error("build openai provider: {0}")]
    BuildOpenAi(ProvideError),

    #[error("build deepseek provider: {0}")]
    BuildDeepSeek(ProvideError),

    #[error("build anthropic provider: {0}")]
    BuildAnthropic(ProvideError),
}

pub trait IntoChatMessages {
    fn into_messages(self) -> Vec<ChatMessage>;
}

impl IntoChatMessages for String {
    #[inline]
    fn into_messages(self) -> Vec<ChatMessage> {
        vec![ChatMessage::user(self)]
    }
}

impl IntoChatMessages for &String {
    #[inline]
    fn into_messages(self) -> Vec<ChatMessage> {
        vec![ChatMessage::user(self)]
    }
}

impl IntoChatMessages for &str {
    #[inline]
    fn into_messages(self) -> Vec<ChatMessage> {
        vec![ChatMessage::user(self)]
    }
}

impl IntoChatMessages for UserMessage {
    #[inline]
    fn into_messages(self) -> Vec<ChatMessage> {
        vec![ChatMessage::User(self)]
    }
}

impl IntoChatMessages for Vec<ChatMessage> {
    #[inline]
    fn into_messages(self) -> Vec<ChatMessage> {
        self
    }
}

impl IntoChatMessages for &[ChatMessage] {
    #[inline]
    fn into_messages(self) -> Vec<ChatMessage> {
        self.to_vec()
    }
}

impl IntoChatMessages for &Vec<ChatMessage> {
    #[inline]
    fn into_messages(self) -> Vec<ChatMessage> {
        self.clone()
    }
}

mod privacy;
pub use privacy::*;
mod tool;
pub use tool::*;
mod sysprompt;
pub use sysprompt::*;
use std::fmt::Display;
use abu_base::chat::{AssistantMessage, ChatMessage, ToolCall};
use abu_tool::ToolCallResult;
use crate::{AgentError, AgentResult};

// ================================================================================ // 
//                             Core Middleware
// ================================================================================ // 

pub enum MiddlewareFlow {
    Continue,
    Break(String),
}

#[async_trait::async_trait]
pub trait SystemPromptMiddleware: Send + Sync {
    type Error: Display + Send + Sync + 'static;
    async fn intercept(&mut self, prompt: &mut String) -> Result<MiddlewareFlow, Self::Error>;
}

/// Before llm generate
#[async_trait::async_trait]
pub trait LlmInputMiddleware: Send + Sync {
    type Error: Display + Send + Sync + 'static;
    async fn intercept(&mut self, messages: &mut Vec<ChatMessage>) -> Result<MiddlewareFlow, Self::Error>;
}

/// After llm generate
#[async_trait::async_trait]
pub trait LlmOutMiddleware: Send + Sync {
    type Error: Display + Send + Sync + 'static;
    async fn intercept(&mut self, ai_message: &mut AssistantMessage) -> Result<MiddlewareFlow, Self::Error>;
}

/// Before tool call
#[async_trait::async_trait]
pub trait ToolCallMiddleware: Send + Sync {
    type Error: Display + Send + Sync + 'static;
    async fn intercept(&mut self, tool_call: &mut ToolCall) -> Result<MiddlewareFlow, Self::Error>;
}

/// After tool call
#[async_trait::async_trait]
pub trait ToolResultMiddleware: Send + Sync {
    type Error: Display + Send + Sync + 'static;
    async fn intercept(&mut self, tool_name: &str, result: &mut ToolCallResult) -> Result<MiddlewareFlow, Self::Error>;
}

/// Before add memory
#[async_trait::async_trait]
pub trait MemoryAddMiddleware: Send + Sync {
    type Error: Display + Send + Sync + 'static;
    async fn intercept(&mut self, user_input: &str, ai_response: &mut String) -> Result<MiddlewareFlow, Self::Error>;
}

// ================================================================================ // 
//                             Wrap Middleware
// ================================================================================ // 

pub enum Middleware {
    SystemPrompt(Box<dyn DynSystemPromptMiddleware>),
    LlmInput(Box<dyn DynLlmInputMiddleware>),
    LlmOut(Box<dyn DynLlmOutMiddleware>),
    ToolCall(Box<dyn DynToolCallMiddleware>),
    ToolResult(Box<dyn DynToolResultMiddleware>),
    MemoryAdd(Box<dyn DynMemoryAddMiddleware>),
} 

impl Middleware {
    pub fn system_prompt<M: SystemPromptMiddleware + 'static>(m: M) -> Self {
        Self::SystemPrompt(Box::new(m))
    }

    pub fn llm_input<M: LlmInputMiddleware + 'static>(m: M) -> Self {
        Self::LlmInput(Box::new(m))
    }

    pub fn llm_out<M: LlmOutMiddleware + 'static>(m: M) -> Self {
        Self::LlmOut(Box::new(m))
    }

    pub fn tool_call<M: ToolCallMiddleware + 'static>(m: M) -> Self {
        Self::ToolCall(Box::new(m))
    }

    pub fn tool_result<M: ToolResultMiddleware + 'static>(m: M) -> Self {
        Self::ToolResult(Box::new(m))
    }  
}

#[derive(Default)]
pub struct MiddlewareManager {
    system_prompts: Vec<Box<dyn DynSystemPromptMiddleware>>,
    llm_inputs: Vec<Box<dyn DynLlmInputMiddleware>>,
    llm_outs: Vec<Box<dyn DynLlmOutMiddleware>>,
    tool_calls: Vec<Box<dyn DynToolCallMiddleware>>,
    tool_results: Vec<Box<dyn DynToolResultMiddleware>>,
    memory_adds: Vec<Box<dyn DynMemoryAddMiddleware>>,
}

macro_rules! pass_middleware_flow {
    ($flow:ident) => {
        if matches!($flow, MiddlewareFlow::Break(_)) {
            return Ok($flow);
        }
    };
}

impl MiddlewareManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn intercept_system_prompt(&mut self, prompt: &mut String) -> AgentResult<MiddlewareFlow> {
        for middleware in self.system_prompts.iter_mut() {
            let flow = middleware.intercept(prompt).await?;
            pass_middleware_flow!(flow);
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub async fn intercept_llm_input(&mut self, messages: &mut Vec<ChatMessage>) -> AgentResult<MiddlewareFlow> {
        for middleware in self.llm_inputs.iter_mut() {
            let flow = middleware.intercept(messages).await?;
            pass_middleware_flow!(flow);
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub async fn intercept_llm_out(&mut self, ai_message: &mut AssistantMessage) -> AgentResult<MiddlewareFlow> {
        for middleware in self.llm_outs.iter_mut() {
            let flow = middleware.intercept(ai_message).await?;
            pass_middleware_flow!(flow);
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub async fn intercept_tool_call(&mut self, tool_call: &mut ToolCall) -> AgentResult<MiddlewareFlow> {
        for middleware in self.tool_calls.iter_mut() {
            let flow = middleware.intercept(tool_call).await?;
            pass_middleware_flow!(flow);
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub async fn intercept_tool_result(&mut self, tool_name: &str, result: &mut ToolCallResult) -> AgentResult<MiddlewareFlow> {
        for middleware in self.tool_results.iter_mut() {
            let flow = middleware.intercept(tool_name, result).await?;
            pass_middleware_flow!(flow);
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub async fn intercept_memory_add(&mut self, user_input: &str, ai_response: &mut String) -> AgentResult<MiddlewareFlow> {
        for middleware in self.memory_adds.iter_mut() {
            let flow = middleware.intercept(user_input, ai_response).await?;
            pass_middleware_flow!(flow);
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub fn add_system_prompt<M: SystemPromptMiddleware + 'static>(&mut self, middleware: M) {
        self.system_prompts.push(Box::new(middleware));
    }

    pub fn add_llm_input<M: LlmInputMiddleware + 'static>(&mut self, middleware: M) {
        self.llm_inputs.push(Box::new(middleware));
    }

    pub fn add_llm_out<M: LlmOutMiddleware + 'static>(&mut self, middleware: M) {
        self.llm_outs.push(Box::new(middleware));
    }

    pub fn add_tool_call<M: ToolCallMiddleware + 'static>(&mut self, middleware: M) {
        self.tool_calls.push(Box::new(middleware));
    }

    pub fn add_tool_result<M: ToolResultMiddleware + 'static>(&mut self, middleware: M) {
        self.tool_results.push(Box::new(middleware));
    }

    pub fn add_memory_add<M: MemoryAddMiddleware + 'static>(&mut self, middleware: M) {
        self.memory_adds.push(Box::new(middleware));
    }

    pub fn add_middleware(&mut self, middleware: impl Into<Middleware>) {
        match middleware.into() {
            Middleware::SystemPrompt(m) => self.system_prompts.push(m),
            Middleware::LlmInput(m) => self.llm_inputs.push(m),
            Middleware::LlmOut(m) => self.llm_outs.push(m),
            Middleware::ToolCall(m) => self.tool_calls.push(m),
            Middleware::ToolResult(m) => self.tool_results.push(m),
            Middleware::MemoryAdd(m) => self.memory_adds.push(m),
        }
    }
}

// ======================================================================================= //
//                   Dyn trait
// ======================================================================================= //

#[async_trait::async_trait]
pub trait DynSystemPromptMiddleware: Send + Sync {
    async fn intercept(&mut self, prompt: &mut String) -> AgentResult<MiddlewareFlow>;
}

#[async_trait::async_trait]
pub trait DynLlmInputMiddleware: Send + Sync {
    async fn intercept(&mut self, messages: &mut Vec<ChatMessage>) -> AgentResult<MiddlewareFlow>;
}

#[async_trait::async_trait]
pub trait DynLlmOutMiddleware: Send + Sync {
    async fn intercept(&mut self, ai_message: &mut AssistantMessage) -> AgentResult<MiddlewareFlow>;
}

#[async_trait::async_trait]
pub trait DynToolCallMiddleware: Send + Sync {
    async fn intercept(&mut self, tool_call: &mut ToolCall) -> AgentResult<MiddlewareFlow>;
}

#[async_trait::async_trait]
pub trait DynToolResultMiddleware: Send + Sync {
    async fn intercept(&mut self, tool_name: &str, result: &mut ToolCallResult) -> AgentResult<MiddlewareFlow>;
}

#[async_trait::async_trait]
pub trait DynMemoryAddMiddleware: Send + Sync {
    async fn intercept(&mut self, user_input: &str, ai_response: &mut String) -> AgentResult<MiddlewareFlow>;
}

#[async_trait::async_trait]
impl<M: SystemPromptMiddleware> DynSystemPromptMiddleware for M {
    #[inline]
    async fn intercept(&mut self, prompt: &mut String) -> AgentResult<MiddlewareFlow> {
        let res = self
            .intercept(prompt).await
            .map_err(|e| AgentError::Middleware("system prompt", e.to_string()))?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<M: LlmInputMiddleware> DynLlmInputMiddleware for M {
    #[inline]
    async fn intercept(&mut self, messages: &mut Vec<ChatMessage>) -> AgentResult<MiddlewareFlow> {
        let res = self
            .intercept(messages).await
            .map_err(|e| AgentError::Middleware("llm input", e.to_string()))?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<M: LlmOutMiddleware> DynLlmOutMiddleware for M {
    #[inline]
    async fn intercept(&mut self, ai_message: &mut AssistantMessage) -> AgentResult<MiddlewareFlow> {
        let res = self
            .intercept(ai_message).await
            .map_err(|e| AgentError::Middleware("llm out", e.to_string()))?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<M: ToolCallMiddleware> DynToolCallMiddleware for M {
    #[inline]
    async fn intercept(&mut self, tool_call: &mut ToolCall) -> AgentResult<MiddlewareFlow> {
        let res = self
            .intercept(tool_call).await
            .map_err(|e| AgentError::Middleware("tool call", e.to_string()))?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<M: ToolResultMiddleware> DynToolResultMiddleware for M {
    #[inline]
    async fn intercept(&mut self, tool_name: &str, result: &mut ToolCallResult) -> AgentResult<MiddlewareFlow> {
        let res = self
            .intercept(tool_name, result).await
            .map_err(|e| AgentError::Middleware("tool result", e.to_string()))?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<M: MemoryAddMiddleware> DynMemoryAddMiddleware for M {
    #[inline]
    async fn intercept(&mut self, user_input: &str, ai_response: &mut String) -> AgentResult<MiddlewareFlow> {
        let res = self
            .intercept(user_input, ai_response).await
            .map_err(|e| AgentError::Middleware("memory add", e.to_string()))?;
        Ok(res)
    }
}

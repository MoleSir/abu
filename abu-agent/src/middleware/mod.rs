mod hitl;
pub use hitl::*;

use abu_base::chat::{AssistantMessage, ToolCall};
use abu_tool::ToolCallResult;
use crate::{AgentError, AgentResult};

pub enum MiddlewareFlow<T> {
    Continue,
    Break(T),
}

#[async_trait::async_trait]
pub trait LlmOutMiddleware: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn intercept(&self, ai_message: &mut AssistantMessage) -> Result<MiddlewareFlow<String>, Self::Error>;
}

#[async_trait::async_trait]
pub trait ToolCallMiddleware: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn intercept(&self, tool_call: &mut ToolCall) -> Result<MiddlewareFlow<String>, Self::Error>;
}

#[async_trait::async_trait]
pub trait ToolResultMiddleware: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn intercept(&self, tool_name: &str, result: &mut ToolCallResult) -> Result<MiddlewareFlow<String>, Self::Error>;
}


pub enum Middleware {
    LlmOut(Box<dyn DynLlmOutMiddleware>),
    ToolCall(Box<dyn DynToolCallMiddleware>),
    ToolResult(Box<dyn DynToolResultMiddleware>),
} 

impl Middleware {
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
    llm_outs: Vec<Box<dyn DynLlmOutMiddleware>>,
    tool_calls: Vec<Box<dyn DynToolCallMiddleware>>,
    tool_results: Vec<Box<dyn DynToolResultMiddleware>>,
}

impl MiddlewareManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn intercept_llm_out(&self, ai_message: &mut AssistantMessage) -> AgentResult<MiddlewareFlow<String>> {
        for llm_out in self.llm_outs.iter() {
            llm_out.intercept(ai_message).await?;
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub async fn intercept_tool_call(&self, tool_call: &mut ToolCall) -> AgentResult<MiddlewareFlow<String>> {
        for llm_out in self.tool_calls.iter() {
            llm_out.intercept(tool_call).await?;
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub async fn intercept_tool_result(&self, tool_name: &str, result: &mut ToolCallResult) -> AgentResult<MiddlewareFlow<String>> {
        for llm_out in self.tool_results.iter() {
            llm_out.intercept(tool_name, result).await?;
        }
        Ok(MiddlewareFlow::Continue)
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

    pub fn add_middleware(&mut self, middleware: impl Into<Middleware>) {
        match middleware.into() {
            Middleware::LlmOut(m) => self.llm_outs.push(m),
            Middleware::ToolCall(m) => self.tool_calls.push(m),
            Middleware::ToolResult(m) => self.tool_results.push(m),
        }
    }
}

// ======================================================================================= //
//                   Dyn trait
// ======================================================================================= //

#[async_trait::async_trait]
pub trait DynLlmOutMiddleware: Send + Sync {
    async fn intercept(&self, ai_message: &mut AssistantMessage) -> AgentResult<MiddlewareFlow<String>>;
}

#[async_trait::async_trait]
pub trait DynToolCallMiddleware: Send + Sync {
    async fn intercept(&self, tool_call: &mut ToolCall) -> AgentResult<MiddlewareFlow<String>>;
}

#[async_trait::async_trait]
pub trait DynToolResultMiddleware: Send + Sync {
    async fn intercept(&self, tool_name: &str, result: &mut ToolCallResult) -> AgentResult<MiddlewareFlow<String>>;
}

#[async_trait::async_trait]
impl<M: LlmOutMiddleware> DynLlmOutMiddleware for M {
    #[inline]
    async fn intercept(&self, ai_message: &mut AssistantMessage) -> AgentResult<MiddlewareFlow<String>> {
        let res = self
            .intercept(ai_message).await
            .map_err(|e| AgentError::Middleware("llm out", Box::new(e)))?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<M: ToolCallMiddleware> DynToolCallMiddleware for M {
    #[inline]
    async fn intercept(&self, tool_call: &mut ToolCall) -> AgentResult<MiddlewareFlow<String>> {
        let res = self
            .intercept(tool_call).await
            .map_err(|e| AgentError::Middleware("tool call", Box::new(e)))?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<M: ToolResultMiddleware> DynToolResultMiddleware for M {
    #[inline]
    async fn intercept(&self, tool_name: &str, result: &mut ToolCallResult) -> AgentResult<MiddlewareFlow<String>> {
        let res = self
            .intercept(tool_name, result).await
            .map_err(|e| AgentError::Middleware("tool result", Box::new(e)))?;
        Ok(res)
    }
}

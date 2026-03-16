mod privacy;
pub use privacy::*;
mod tool;
pub use tool::*;

use abu_base::chat::{AssistantMessage, ToolCall};
use abu_tool::ToolCallResult;
use crate::{AgentError, AgentResult};

pub enum MiddlewareFlow {
    Continue,
    Break(String),
}

#[async_trait::async_trait]
pub trait LlmOutMiddleware: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn intercept(&self, ai_message: &mut AssistantMessage) -> Result<MiddlewareFlow, Self::Error>;
}

#[async_trait::async_trait]
pub trait ToolCallMiddleware: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn intercept(&self, tool_call: &mut ToolCall) -> Result<MiddlewareFlow, Self::Error>;
}

#[async_trait::async_trait]
pub trait ToolResultMiddleware: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn intercept(&self, tool_name: &str, result: &mut ToolCallResult) -> Result<MiddlewareFlow, Self::Error>;
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

    pub async fn intercept_llm_out(&self, ai_message: &mut AssistantMessage) -> AgentResult<MiddlewareFlow> {
        for middleware in self.llm_outs.iter() {
            let flow = middleware.intercept(ai_message).await?;
            pass_middleware_flow!(flow);
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub async fn intercept_tool_call(&self, tool_call: &mut ToolCall) -> AgentResult<MiddlewareFlow> {
        for middleware in self.tool_calls.iter() {
            let flow = middleware.intercept(tool_call).await?;
            pass_middleware_flow!(flow);
        }
        Ok(MiddlewareFlow::Continue)
    }

    pub async fn intercept_tool_result(&self, tool_name: &str, result: &mut ToolCallResult) -> AgentResult<MiddlewareFlow> {
        for middleware in self.tool_results.iter() {
            let flow = middleware.intercept(tool_name, result).await?;
            pass_middleware_flow!(flow);
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
    async fn intercept(&self, ai_message: &mut AssistantMessage) -> AgentResult<MiddlewareFlow>;
}

#[async_trait::async_trait]
pub trait DynToolCallMiddleware: Send + Sync {
    async fn intercept(&self, tool_call: &mut ToolCall) -> AgentResult<MiddlewareFlow>;
}

#[async_trait::async_trait]
pub trait DynToolResultMiddleware: Send + Sync {
    async fn intercept(&self, tool_name: &str, result: &mut ToolCallResult) -> AgentResult<MiddlewareFlow>;
}

#[async_trait::async_trait]
impl<M: LlmOutMiddleware> DynLlmOutMiddleware for M {
    #[inline]
    async fn intercept(&self, ai_message: &mut AssistantMessage) -> AgentResult<MiddlewareFlow> {
        let res = self
            .intercept(ai_message).await
            .map_err(|e| AgentError::Middleware("llm out", Box::new(e)))?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<M: ToolCallMiddleware> DynToolCallMiddleware for M {
    #[inline]
    async fn intercept(&self, tool_call: &mut ToolCall) -> AgentResult<MiddlewareFlow> {
        let res = self
            .intercept(tool_call).await
            .map_err(|e| AgentError::Middleware("tool call", Box::new(e)))?;
        Ok(res)
    }
}

#[async_trait::async_trait]
impl<M: ToolResultMiddleware> DynToolResultMiddleware for M {
    #[inline]
    async fn intercept(&self, tool_name: &str, result: &mut ToolCallResult) -> AgentResult<MiddlewareFlow> {
        let res = self
            .intercept(tool_name, result).await
            .map_err(|e| AgentError::Middleware("tool result", Box::new(e)))?;
        Ok(res)
    }
}

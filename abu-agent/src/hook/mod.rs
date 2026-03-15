mod consolelog;
pub use consolelog::*;

use abu_base::chat::{AssistantMessage, ChatMessage, ToolCall};
use abu_tool::ToolCallResult;
use crate::AgentError;

/// All runtime events emitted by Agent
/// 
/// AgentStart
///  1. MemorySearch
///  2. ContextBuild
///  3. StepStart
///      1. LlmStart
///      2. LlmEnd
///      3. ToolStart
///      4. ToolEnd
///  4. StepEnd
/// AgentEnd
pub enum HookEvent<'a> {
    // ===== agent lifecycle =====
    AgentStart { query: &'a str, },
    AgentEnd { result: &'a str, },
    AgentMaxIteration,
    AgentStepStart { step: usize, },
    AgentStepEnd { step: usize, message: &'a AssistantMessage,},

    // ===== context =====
    ContextBuild { query: &'a str, messages: &'a [ChatMessage] },

    // ===== memory =====
    MemorySearch { query: &'a str, results: &'a [ChatMessage] },
    MemoryAdd { user: &'a str, assistant: &'a str },

    // ===== llm =====
    LlmStart { step: usize, messages: &'a [ChatMessage] },
    LlmEnd { step: usize, message: &'a AssistantMessage },

    // ===== tool =====
    ToolStart { step: usize, tool_call: &'a ToolCall },
    ToolEnd { step: usize, result: &'a ToolCallResult },
    ToolError { step: usize, context: &'a str },

    // ===== error =====
    Error { error: &'a AgentError },
}

#[async_trait::async_trait]
#[allow(unused)]
pub trait Hook: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn on_event(&self, event: HookEvent<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl<'a> HookEvent<'a> {
    pub fn agent_start(query: &'a str) -> Self {
        Self::AgentStart { query }
    }

    pub fn agent_end(result: &'a str) -> Self {
        Self::AgentEnd { result }
    }

    pub fn agent_max_iteration() -> Self {
        Self::AgentMaxIteration
    }

    pub fn step_start(step: usize) -> Self {
        Self::AgentStepStart { step }
    }

    pub fn step_end(step: usize, message: &'a AssistantMessage) -> Self {
        Self::AgentStepEnd { step, message }
    }

    pub fn context_build(query: &'a str, messages: &'a [ChatMessage]) -> Self {
        Self::ContextBuild { query, messages }
    }

    pub fn memory_search(query: &'a str, results: &'a [ChatMessage]) -> Self {
        Self::MemorySearch { query, results }
    }

    pub fn memory_add(user: &'a str, assistant: &'a str) -> Self {
        Self::MemoryAdd { user, assistant }
    }

    pub fn llm_start(step: usize, messages: &'a [ChatMessage]) -> Self {
        Self::LlmStart { step, messages }
    }

    pub fn llm_end(step: usize, message: &'a AssistantMessage) -> Self {
        Self::LlmEnd { step, message }
    }

    pub fn tool_start(step: usize, tool_call: &'a ToolCall) -> Self {
        Self::ToolStart { step, tool_call }
    }

    pub fn tool_end(step: usize, result: &'a ToolCallResult) -> Self {
        Self::ToolEnd { step, result }
    }

    pub fn tool_error(step: usize, context: &'a str) -> Self {
        Self::ToolError { step, context }
    }

    pub fn error(error: &'a AgentError) -> Self {
        Self::Error { error }
    }
}

#[async_trait::async_trait]
pub trait HookWrap : Send + Sync {
    async fn on_event(&self, event: HookEvent<'_>) -> Result<(), AgentError>;
} 

#[async_trait::async_trait]
impl<H: Hook> HookWrap for H {
    #[inline]
    async fn on_event(&self, event: HookEvent<'_>) -> Result<(), AgentError> {
        self
            .on_event(event).await
            .map_err(|e| AgentError::Hook(Box::new(e)))
    }
}

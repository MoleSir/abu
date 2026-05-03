mod consolelog;
use std::fmt::Display;

pub use consolelog::*;
mod auditfilelog;
pub use auditfilelog::*;

use abu_base::chat::{AssistantMessage, ChatMessage, ToolCall};
use abu_tool::ToolCallResult;
use serde::Serialize;
use crate::{AgentError, AgentResult};

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
#[derive(Serialize)]
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
}

#[async_trait::async_trait]
pub trait Hook: Send + Sync {
    type Error: Display + Send + Sync + 'static;
    async fn on_event(&self, event: &HookEvent<'_>) -> Result<(), Self::Error>;
}

#[derive(Default)]
pub struct HookManager {
    hooks: Vec<Box<dyn HookWrap>>,
}

impl HookManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_hook<H: Hook + 'static>(&mut self, hook: H) {
        self.hooks.push(Box::new(hook));
    }

    pub async fn on_agent_start(&self, query: &str) -> AgentResult<()> {
        let event = HookEvent::agent_start(query);
        self.dispatch(&event).await
    }

    pub async fn on_agent_end(&self, result: &str) -> AgentResult<()> {
        let event = HookEvent::agent_end(result);
        self.dispatch(&event).await
    }

    pub async fn on_agent_max_iteration(&self) -> AgentResult<()> {
        let event = HookEvent::agent_max_iteration();
        self.dispatch(&event).await
    }

    pub async fn on_step_start(&self, step: usize) -> AgentResult<()> {
        let event = HookEvent::step_start(step);
        self.dispatch(&event).await
    }

    pub async fn on_step_end(&self, step: usize, message: &AssistantMessage) -> AgentResult<()> {
        let event = HookEvent::step_end(step, message);
        self.dispatch(&event).await
    }

    pub async fn on_context_build(&self, query: &str, messages: &[ChatMessage]) -> AgentResult<()> {
        let event = HookEvent::context_build(query, messages);
        self.dispatch(&event).await
    }

    pub async fn on_memory_search(&self, query: &str, results: &[ChatMessage]) -> AgentResult<()> {
        let event = HookEvent::memory_search(query, results);
        self.dispatch(&event).await
    }

    pub async fn on_memory_add(&self, user: &str, assistant: &str) -> AgentResult<()> {
        let event = HookEvent::memory_add(user, assistant);
        self.dispatch(&event).await
    }

    pub async fn on_llm_start(&self, step: usize, messages: &[ChatMessage]) -> AgentResult<()> {
        let event = HookEvent::llm_start(step, messages);
        self.dispatch(&event).await
    }

    pub async fn on_llm_end(&self, step: usize, message: &AssistantMessage) -> AgentResult<()> {
        let event = HookEvent::llm_end(step, message);
        self.dispatch(&event).await
    }

    pub async fn on_tool_start(&self, step: usize, tool_call: &ToolCall) -> AgentResult<()> {
        let event = HookEvent::tool_start(step, tool_call);
        self.dispatch(&event).await
    }

    pub async fn on_tool_end(&self, step: usize, result: &ToolCallResult) -> AgentResult<()> {
        let event = HookEvent::tool_end(step, result);
        self.dispatch(&event).await
    }

    pub async fn on_tool_error(&self, step: usize, context: &str) -> AgentResult<()> {
        let event = HookEvent::tool_error(step, context);
        self.dispatch(&event).await
    }

    async fn dispatch(&self, event: &HookEvent<'_>) -> AgentResult<()> {
        for hook in &self.hooks {
            hook.on_event(event).await?;
        }
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
}

#[async_trait::async_trait]
pub trait HookWrap : Send + Sync {
    async fn on_event(&self, event: &HookEvent<'_>) -> Result<(), AgentError>;
} 

#[async_trait::async_trait]
impl<H: Hook> HookWrap for H {
    #[inline]
    async fn on_event(&self, event: &HookEvent<'_>) -> Result<(), AgentError> {
        self
            .on_event(event).await
            .map_err(|e| AgentError::Hook(e.to_string()))
    }
}

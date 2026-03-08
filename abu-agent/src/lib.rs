pub mod error;
use context::ContextBuilder;
pub use error::*;
use memory::Memory;

pub mod llm;
pub mod kit;
pub mod memory;
pub mod context;
pub mod prompt;
pub mod build;

use std::sync::Arc;

pub use build::AgentBuilder;
use abu_api::chat::{ChatMessage, ToolDefinition};
use thiserrorctx::Context;
use tokio::sync::{RwLockReadGuard, RwLock};
use crate::{kit::AgentKit, llm::LLM};
use tracing::{info, warn};

#[derive(Clone)]
pub struct AgentConfig {
    pub max_iteration: usize,
    pub temperature: f64,
}

pub struct Agent<M: Memory> {
    pub config: AgentConfig,
    pub llm: Arc<LLM>,
    pub memory: M,
    pub context_builder: ContextBuilder,
    pub kit: Arc<RwLock<AgentKit>>,
}

impl<M: Memory> Agent<M> {
    pub async fn tool_list(&self) -> RwLockReadGuard<'_, [ToolDefinition]> {
        let gurad = self.kit.read().await;
        RwLockReadGuard::map(gurad, |kit| kit.tool_definitions())
    }

    pub fn system_prompt(&self) -> &str {
        &self.context_builder.system_prompt
    }

    pub async fn run(&mut self, query: &str) -> AgentResult<String> {
        info!(query = %query, "🤖 Agent started with user query");

        // compact the history
        let memorys: Vec<ChatMessage> = self.memory.search(query).await
            .map_err(|e| AgentError::Memory(Box::new(e)))
            .context("search memory")?;
        let mut messages: Vec<ChatMessage> = self.context_builder.build(query, memorys);

        // agent loop
        let mut final_result = None; 
        for step in 0..self.config.max_iteration {
            info!(step, "🔄 Agent step begin");
            let response = self.llm
                .chat(&messages, self.kit.read().await.tool_definitions(), self.config.temperature)
                .await
                .context("chat with llm")?;

            // insert ai response
            messages.push(response.clone().into());

            info!(step, role = "AI", content = response.content, "🗣️ LLM Text Response");
            if !response.tool_calls.is_empty() {
                info!(step, count = response.tool_calls.len(), "🛠️ LLM requested tool calls");
            } else {
                final_result = Some(response.content);
                break;
            }

            // tool calls
            let mut terminate_message = None;
            for tool_call in response.tool_calls.iter() {
                info!(step, tool = %tool_call.function.name, id = %tool_call.id, args = %tool_call.function.arguments, "🚀 Executing tool");

                let result = self.kit.write().await.execute_tool(tool_call).await.context("execute tool")?;
                info!(step,tool = %tool_call.function.name, result = %result, "✅ Tool execution finished");

                // save terminate message
                if tool_call.function.name == "terminate" {
                    terminate_message = Some(result.clone());
                }

                // insert tool response
                messages.push(ChatMessage::tool(result, tool_call.id.clone()));
            }

            if let Some(terminate_message) = terminate_message {
                info!(step, "🛑 Agent terminated by tool");
                final_result = Some(terminate_message);
                break;
            }
        }

        match final_result {
            Some(final_result) => {
                self.memory.add(query, &final_result).await
                    .map_err(|e| AgentError::Memory(Box::new(e)))
                    .context("add new memory")?;
                Ok(final_result)
            }
            None => {
                warn!("Agent reached max steps without termination");
                Ok("Task do not finish yet".to_string())
            }
        }
    }

    pub async fn chat(&mut self, query: &str) -> AgentResult<String> {
        info!(query = %query, "🤖 Agent started with user query");

        // compact the history
        let memorys: Vec<ChatMessage> = self.memory.search(query).await
            .map_err(|e| AgentError::Memory(Box::new(e)))
            .context("search memory")?;
        let messages: Vec<ChatMessage> = self.context_builder.build(query, memorys);

        let response = self.llm
            .chat(&messages, &[], self.config.temperature)
            .await
            .context("chat with llm")?;

        info!(role = "AI", content = response.content, "🗣️ LLM Text Response");
            
        self.memory.add(query, &response.content).await
            .map_err(|e| AgentError::Memory(Box::new(e)))
            .context("add new memory")?;
        Ok(response.content)
    }
}
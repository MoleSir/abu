pub mod error;
pub use error::*;
use history::AgentHistory;

pub mod llm;
pub mod kit;
pub mod history;
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

pub struct Agent {
    pub config: AgentConfig,
    pub llm: Arc<LLM>,
    pub history: AgentHistory,
    pub kit: Arc<RwLock<AgentKit>>,
}

impl Agent {
    pub async fn tool_list(&self) -> RwLockReadGuard<'_, [ToolDefinition]> {
        let gurad = self.kit.read().await;
        RwLockReadGuard::map(gurad, |kit| kit.tool_definitions())
    }

    pub fn system_prompt(&self) -> &str {
        &self.history.system_prompt
    }

    pub async fn run(&mut self, query: &str) -> AgentResult<String> {
        info!(query = %query, "🤖 Agent started with user query");

        // compact the history
        self.history.compact(query).await.context("load messages")?;

        // insert query message
        self.history.add_message(ChatMessage::user(query)).await.context("add query messages")?;

        // agent loop
        for step in 0..self.config.max_iteration {
            info!(step, "🔄 Agent step begin");
            let response = self.llm
                .chat(self.history.messages(), self.kit.read().await.tool_definitions(), self.config.temperature)
                .await
                .context("chat with llm")?;

            // insert ai response
            self.history.add_message(response.clone().into()).await.context("add ai messages")?;

            if !response.content.is_empty() {
                info!(step, role = "AI", content = response.content, "🗣️ LLM Text Response");
            }

            if !response.tool_calls.is_empty() {
                info!(step, count = response.tool_calls.len(), "🛠️ LLM requested tool calls");
            } else {
                return Ok("finish task".to_string())
            }

            // tool calls
            let mut terminate = None;
            for tool_call in response.tool_calls.iter() {
                info!(step, tool = %tool_call.function.name, id = %tool_call.id, args = %tool_call.function.arguments, "🚀 Executing tool");

                let result = self.kit.write().await.execute_tool(tool_call).await.context("execute tool")?;
                info!(step,tool = %tool_call.function.name, result = %result, "✅ Tool execution finished");

                // save terminate message
                if tool_call.function.name == "terminate" {
                    terminate = Some(result.clone());
                }

                // insert tool response
                let tool_msg = ChatMessage::tool(result, tool_call.id.clone());
                self.history.add_message(tool_msg).await.context("add tool messages")?;
            }

            if let Some(terminate) = terminate {
                info!(step, "🛑 Agent terminated by tool");
                return Ok(terminate);
            }
        }

        warn!("Agent reached max steps without termination");
        Ok("Task do not finish yet".to_string())
    }
}
mod kit;
mod build;
pub use kit::AgentKit;
pub use build::AgentBuilder;
use abu_api::chat::{ChatMessage, ToolDefinition};
use thiserrorctx::Context;
use crate::{llm::LLM, memory::Memory, AgentResult};
use tracing::{info, warn};

pub struct AgentConfig {
    pub max_iteration: usize,
    pub temperature: f64,
}

pub struct Agent {
    config: AgentConfig,
    llm: LLM,
    memory: Memory,
    kit: AgentKit,
}

impl Agent {
    #[inline]
    pub fn tool_list(&self) -> &[ToolDefinition] {
        self.kit.tool_definitions()
    }

    pub fn system_prompt(&self) -> &str {
        &self.memory.system_prompt
    }

    pub async fn run(&mut self, query: &str) -> AgentResult<()> {
        info!(query = %query, "🤖 Agent started with user query");

        // compact the history
        let mut history = self.memory.load_messages(query).await.context("load messages")?;

        // insert query message
        let user_msg = ChatMessage::user(query);
        self.memory.add_message(user_msg.clone()).await.context("add query messages")?;
        history.push(user_msg); 

        for step in 0..self.config.max_iteration {
            info!(step, "🔄 Agent step begin");
            let response = self.llm
                .chat(&history, self.kit.tool_definitions(), self.config.temperature)
                .await
                .context("chat with llm")?;

            if !response.content.is_empty() {
                info!(step, role = "AI", content = response.content, "🗣️ LLM Text Response");
            }

            if !response.tool_calls.is_empty() {
                info!(step, count = response.tool_calls.len(), "🛠️ LLM requested tool calls");
            }
            
            // insert ai response
            self.memory.add_message(response.clone().into()).await.context("add ai messages")?;
            history.push(response.clone().into());

            // tool calls
            let mut should_terminate = false;
            for tool_call in response.tool_calls.iter() {
                info!(step, tool = %tool_call.function.name, id = %tool_call.id, args = %tool_call.function.arguments, "🚀 Executing tool");

                if tool_call.function.name == "terminate" {
                    should_terminate = true;
                }

                let result = self.kit.execute_tool(tool_call).await.context("execute tool")?;
                info!(step,tool = %tool_call.function.name, result = %result, "✅ Tool execution finished");

                // insert tool response
                let tool_msg = ChatMessage::tool(result, tool_call.id.clone());
                self.memory.add_message(tool_msg.clone()).await.context("add tool messages")?;
                history.push(tool_msg);
            }

            if should_terminate {
                info!(step, "🛑 Agent terminated by tool");
                return Ok(());
            }
        }

        warn!("Agent reached max steps without termination");
        Ok(())
    }
}
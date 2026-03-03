mod kit;
mod build;
pub use kit::AgentKit;
pub use build::AgentBuilder;
use abu_api::chat::{ChatMessage, ToolDefinition};
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
    pub fn tool_list(&self) -> &[ToolDefinition] {
        self.kit.tool_definitions()
    }

    pub async fn run(&mut self, query: &str) -> AgentResult<()> {
        info!(query = %query, "🤖 Agent started with user query");

        self.memory.add_message(ChatMessage::user(query)).await?;

        for step in 0..self.config.max_iteration {
            info!(step, "🔄 Agent step begin");
            let messages = self.memory.load_messages(query).await?;
            let response = self.llm.chat(messages, self.kit.tool_definitions(), self.config.temperature).await?;

            if !response.content.is_empty() {
                info!(step, role = "AI", content = response.content, "🗣️ LLM Text Response");
            }

            if !response.tool_calls.is_empty() {
                info!(step, count = response.tool_calls.len(), "🛠️ LLM requested tool calls");
            }

            self.memory.add_message(response.clone().into()).await?;

            for tool_call in response.tool_calls.iter() {
                info!(step, tool = %tool_call.function.name, id = %tool_call.id, args = %tool_call.function.arguments, "🚀 Executing tool");

                if tool_call.function.name == "terminate" {
                    info!(step, "🛑 Agent terminated by tool");
                    return Ok(());
                }

                let result = self.kit.execute_tool(tool_call).await?;
                info!(step,tool = %tool_call.function.name, result = %result, "✅ Tool execution finished");

                self.memory.add_message(ChatMessage::tool(result, tool_call.id.clone())).await?;
            }
        }

        warn!("Agent reached max steps without termination");
        Ok(())
    }
}
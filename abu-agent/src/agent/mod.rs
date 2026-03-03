mod kit;
mod build;
pub use kit::AgentKit;
pub use build::AgentBuilder;
use abu_api::chat::ChatMessage;
use crate::{llm::LLM, memory::Memory, AgentResult};
use tracing::{info, warn};

pub struct Agent {
    llm: LLM,
    memory: Memory,
    kit: AgentKit,
}

impl Agent {
    pub async fn run(&mut self, query: &str) -> AgentResult<()> {
        info!("agent started");

        self.memory.add_message(ChatMessage::user(query)).await?;

        for step in 0..10 {
            info!(step, "agent step begin");

            let messages = self.memory.load_messages(query).await?;
            info!(step, msg_count = messages.len(), "loaded memory");

            info!(step, "sending request to llm");
            let response = self
                .llm
                .chat(messages, self.kit.tool_definitions(), 0.7)
                .await?;

            info!(
                step,
                tool_calls = response.tool_calls.len(),
                "received llm response"
            );

            self.memory.add_message(response.clone().into()).await?;

            for tool_call in response.tool_calls.iter() {
                info!(
                    step,
                    tool = %tool_call.function.name,
                    call_id = %tool_call.id,
                    "executing tool"
                );

                if tool_call.function.name == "terminate" {
                    info!(step, "agent terminated by tool");
                    return Ok(());
                }

                let result = self.kit.execute_tool(tool_call).await?;

                info!(
                    step,
                    tool = %tool_call.function.name,
                    result_len = result.len(),
                    "tool execution finished"
                );

                self.memory
                    .add_message(ChatMessage::tool(result, tool_call.id.clone()))
                    .await?;
            }
        }

        warn!("agent reached max steps without termination");
        Ok(())
    }
}
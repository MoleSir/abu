pub mod error;
use abu_provider::ChatProvide;
use abu_tool::ToolCallResult;
use context::ContextBuilder;
pub use error::*;
use memory::Memory;

pub mod kit;
pub mod memory;
pub mod context;
pub mod prompt;
pub mod build;

pub use build::AgentBuilder;
use abu_base::chat::{AssistantMessage, ChatMessage, ChatRequest, ChatRequestBuilder, ToolCall, ToolDefinition};
use thiserrorctx::Context;
use crate::kit::AgentKit;
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct AgentConfig {
    pub max_iteration: usize,
    pub temperature: f64,
}

pub struct Agent<C: ChatProvide, M: Memory> {
    pub config: AgentConfig,
    pub llm: C,
    pub model: String,
    pub memory: M,
    pub context_builder: ContextBuilder,
    pub kit: AgentKit,
}

impl<C: ChatProvide, M: Memory> Agent<C, M> {
    // pub async fn tool_list(&self) -> RwLockReadGuard<'_, [ToolDefinition]> {
    //     let gurad = self.kit.read().await;
    //     RwLockReadGuard::map(gurad, |kit| kit.tool_definitions())
    // }

    pub fn tool_list(&self) -> &[ToolDefinition] {
        self.kit.tool_definitions()
    }

    pub fn system_prompt(&self) -> &str {
        &self.context_builder.system_prompt
    }

    pub async fn run(&mut self, query: &str) -> AgentResult<String> {
        info!(query = %query, "🤖 Agent started with user query");
        
        let mut request = self.init_chat_request(query, true).await?;

        // agent loop
        let mut final_result = None; 
        for step in 0..self.config.max_iteration {
            info!(step, "🔄 Agent step begin");
            let ai_message = self.send_chat_request(&request).await?;

            // insert ai response
            request.messages.push(ai_message.clone().into());

            info!(step, role = "AI", content = ai_message.content, "🗣️ LLM Text Response");
            if !ai_message.tool_calls.is_empty() {
                info!(step, count = ai_message.tool_calls.len(), "🛠️ LLM requested tool calls");
            } else {
                final_result = Some(ai_message.content);
                break;
            }

            // tool calls
            for tool_call in ai_message.tool_calls.into_iter() {
                info!(step, tool = %tool_call.name, id = %tool_call.id, args = %tool_call.arguments, "🚀 Executing tool");

                let (id, result) = self.execute_tool(tool_call).await.context("execute tool")?;
                let tool_content = if result.is_error {
                    info!(step, result = %result.context, "Tool execute failed!");
                    format!("Tool execute failed for {}", result.context)
                } else {
                    info!(step, result = %result.context, "✅ Tool execution finished");
                    format!("Tool execute success with output {}", result.context)
                };

                // insert tool response
                request.messages.push(ChatMessage::tool(tool_content, id));
            }
        }

        match final_result {
            Some(final_result) => {
                info!(output = final_result, "🛑 Finsh task with final output");
                self.add_memory(query, &final_result).await?;
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

        let request = self.init_chat_request(query, false).await?;
        let ai_message = self.send_chat_request(&request).await?;

        info!(role = "AI", content = ai_message.content, "🗣️ LLM Text Response");
        self.add_memory(query, &ai_message.content).await?;

        Ok(ai_message.content)
    }

    async fn execute_tool(&mut self, tool_call: ToolCall) -> AgentResult<(String, ToolCallResult)> {
        let id = tool_call.id;
        let name = tool_call.name;
        let arguments = serde_json::from_str(&tool_call.arguments)?;
        let result = self.kit.execute_tool(name, arguments).await.context("execute tool")?;
        Ok((id, result))
    }

    async fn add_memory(&mut self, user_input: &str, ai_response: &str) -> AgentResult<()> {
        debug!(user_input = user_input, ai_response = ai_response, "add memory");
        self.memory
            .add(user_input, ai_response).await
            .map_err(|e| AgentError::Memory(Box::new(e)))
            .context("add new memory")?;
        Ok(())
    }

    async fn send_chat_request(&self, request: &ChatRequest) -> AgentResult<AssistantMessage> {
        let response = self.llm
            .chat(&request).await
            .map_err(|e| AgentError::ChatProvider(Box::new(e)))?;
        Ok(response.message)
    }

    async fn init_chat_request(&mut self, query: &str, with_tool: bool) -> AgentResult<ChatRequest> {
        let memorys: Vec<ChatMessage> = self.memory.search(query).await
            .map_err(|e| AgentError::Memory(Box::new(e)))
            .context("search memory")?;

        let messages: Vec<ChatMessage> = self.context_builder.build(query, memorys);

        let mut builder = ChatRequestBuilder::default();
        builder
            .model(&self.model)
            .messages(messages)
            .temperature(self.config.temperature);
        
        if with_tool {
            builder.tools(self.kit.tool_definitions());
        } 

        let req = builder.build()?;
        Ok(req)
    }
}
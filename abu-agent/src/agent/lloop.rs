use abu_base::chat::{AssistantMessage, ChatMessage, ToolCall, ToolDefinition};
use abu_provider::ChatProvide;
use abu_tool::ToolCallResult;
use crate::{hook::HookEvent, AgentError};
use crate::memory::Memory;
use super::{Agent, AgentResult};

use thiserrorctx::Context;
use tracing::{debug, info, warn};

impl<C: ChatProvide, M: Memory> Agent<C, M> {
    pub fn tool_list(&self) -> &[ToolDefinition] {
        self.kit.tool_definitions()
    }

    pub fn system_prompt(&self) -> &str {
        &self.context_builder.system_prompt
    }

    pub async fn run(&mut self, query: &str) -> AgentResult<String> {
        info!(query = %query, "🤖 Agent started with user query");
        for hook in &self.hooks {
            hook.on_event(HookEvent::agent_start(query)).await.context("agent start hook")?;
        }
        
        let memorys = self.search_memory(query).await.context("search memory")?;
        let mut messages = self.build_context(query, memorys).await.context("build context")?;

        // agent loop
        let mut final_result = None; 
        for step in 0..self.config.max_iteration {
            debug!(step, "🔄 Agent step begin");
            for hook in self.hooks.iter() {
                let event = HookEvent::step_start(step);
                hook.on_event(event).await.with_context(|| format!("step {step} start"))?;
            }

            let ai_message = self.llm_chat(step, &messages, true).await.with_context(|| format!("chat with llm in step {}", step))?;
            messages.push(ai_message.clone().into());

            info!(step, role = "AI", content = ai_message.content, "🗣️ LLM Text Response");
            if !ai_message.tool_calls.is_empty() {
                info!(step, count = ai_message.tool_calls.len(), "🛠️ LLM requested tool calls");
            } else {
                final_result = Some(ai_message.content);
                break;
            }

            // tool calls
            for tool_call in ai_message.tool_calls.iter() {
                info!(step, tool = %tool_call.name, id = %tool_call.id, args = %tool_call.arguments, "🚀 Executing tool");
                let result = self.execute_tool(step, tool_call).await.context("execute tool")?;

                let tool_content = if result.is_error {
                    info!(step, result = %result.context, "Tool execute failed!");
                    format!("Tool execute failed for {}", result.context)
                } else {
                    info!(step, result = %result.context, "✅ Tool execution finished");
                    format!("Tool execute success with output {}", result.context)
                };

                // insert tool response
                messages.push(ChatMessage::tool(tool_content, tool_call.id.clone()));
            }

            debug!(step, "🔄 Agent step end");
            for hook in self.hooks.iter() {
                let event = HookEvent::step_end(step, &ai_message);
                hook.on_event(event).await.with_context(|| format!("step {step} start"))?;
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
                for hook in self.hooks.iter() {
                    hook.on_event(HookEvent::AgentMaxIteration).await.context("max iter hook")?;
                }
                Ok("Task do not finish yet".to_string())
            }
        }
    }

    pub async fn chat(&mut self, query: &str) -> AgentResult<String> {
        info!(query = %query, "🤖 Agent started with user query");

        let memorys = self.search_memory(query).await.context("search memory")?;
        let messages = self.build_context(query, memorys).await.context("build context")?;
        
        let ai_message = self.llm_chat(0, &messages, false).await.context("chat with llm")?;
        info!(role = "AI", content = ai_message.content, "🗣️ LLM Text Response");
        
        self.add_memory(query, &ai_message.content).await?;

        Ok(ai_message.content)
    }

    async fn execute_tool(&mut self, step: usize, tool_call: &ToolCall) -> AgentResult<ToolCallResult> {
        for hook in &self.hooks {
            hook.on_event(HookEvent::tool_start(step, &tool_call)).await.context("tool start hook")?;
        }

        let arguments = serde_json::from_str(&tool_call.arguments)?;
        let result = self.kit.execute_tool(&tool_call.name, arguments).await.context("execute tool")?;
        
        for hook in &self.hooks {
            hook.on_event(HookEvent::tool_end(step, &result)).await.context("tool end hook")?;
        }

        if result.is_error {
            for hook in &self.hooks {
                let event = HookEvent::tool_error(step, &result.context);
                hook.on_event(event).await.context("tool error hook")?;
            }
        }
        
        Ok(result)
    }

    async fn add_memory(&mut self, user_input: &str, ai_response: &str) -> AgentResult<()> {
        debug!(user_input = user_input, ai_response = ai_response, "add memory");
        
        self.memory
            .add(user_input, ai_response).await
            .map_err(|e| AgentError::Memory(Box::new(e)))
            .context("add new memory")?;

        for hook in self.hooks.iter() {
            let event = HookEvent::memory_add(user_input, ai_response);
            hook.on_event(event).await.context("memory add hook")?;
        }

        Ok(())
    }

    async fn search_memory(&mut self, query: &str) -> AgentResult<Vec<ChatMessage>> {
        debug!(query = query, "search memory");
        let memorys: Vec<ChatMessage> = self.memory.search(query).await
            .map_err(|e| AgentError::Memory(Box::new(e)))
            .context("search memory")?;
        for hook in self.hooks.iter() {
            let event = HookEvent::memory_search(query, &memorys);
            hook.on_event(event).await.context("memory search hook")?;
        }

        Ok(memorys)
    }

    async fn build_context(&mut self, query: &str, memorys: Vec<ChatMessage>) -> AgentResult<Vec<ChatMessage>> {
        debug!("build context");
        let messages: Vec<ChatMessage> = self.context_builder.build(query, memorys);
        for hook in self.hooks.iter() {
            let event = HookEvent::context_build(query, &messages);
            hook.on_event(event).await.context("context build hook")?;
        }

        Ok(messages)
    }

    async fn llm_chat(&mut self, step: usize, messages: &[ChatMessage], with_tool: bool) -> AgentResult<AssistantMessage> {
        for hook in &self.hooks {
            let event = HookEvent::llm_start(step, messages);
            hook.on_event(event).await.context("llm start hook")?;
        }

        let ai_message = if with_tool {
            self.llm.chat(messages).await?.message
        } else {
            self.llm.chat_no_tools(messages).await?.message
        };

        for hook in &self.hooks {
            let event = HookEvent::llm_end(step, &ai_message);
            hook.on_event(event).await.context("llm end hook")?;
        }

        Ok(ai_message)
    }
}
use abu_base::chat::{AssistantMessage, ChatMessage, ToolCall, ToolDefinition};
use abu_provider::ChatProvide;
use abu_tool::ToolCallResult;
use crate::{middleware::MiddlewareFlow, AgentError};
use crate::memory::Memory;
use super::{Agent, AgentResult};

use thiserrorctx::Context;
use tracing::{debug, info, warn};

pub enum AgentControl<T> {
    Normal(T),
    Break(String),
}

impl<T> AgentControl<T> {
    pub fn unwrap(self) -> T {
        match self {
            Self::Break(_) => panic!("control is break!"),
            Self::Normal(v) => v,
        }
    }
}

macro_rules! extract_agent_control {
    ($control:ident) => {
        match $control {
            AgentControl::Break(s) => return Ok(AgentControl::Break(s)),
            AgentControl::Normal(m) => m,
        }
    };
}

macro_rules! return_middleware_break {
    ($flow:ident) => {
        if let MiddlewareFlow::Break(s) = $flow {
            return Ok(AgentControl::Break(s));
        }
    };
}

impl<C: ChatProvide, M: Memory> Agent<C, M> {
    pub fn tool_list(&self) -> &[ToolDefinition] {
        self.toolbox.tool_definitions()
    }

    pub fn system_prompt(&self) -> &str {
        &self.context_builder.system_prompt
    }

    pub async fn run(&mut self, query: &str) -> AgentResult<AgentControl<String>> {
        info!(query = %query, "🤖 Agent started with user query");
        self.hooks.on_agent_start(query).await.context("agent start hook")?;
        
        // init context
        let memories = self.search_memory(query).await.context("search memory")?;
        let mut messages = self.build_context(query, memories).await.context("build context")?;

        // agent loop
        let mut final_result = None; 
        for step in 0..self.config.max_iteration {
            debug!(step, "🔄 Agent step begin");
            self.hooks.on_step_start(step).await.with_context(|| format!("step {step} start"))?;

            // chat with llm
            let control = self.llm_chat(step, &mut messages, true).await
                .with_context(|| format!("chat with llm in step {}", step))?;
            let mut ai_message = extract_agent_control!(control);
            messages.push(ai_message.clone().into());

            info!(step, role = "AI", content = ai_message.content, "🗣️ LLM Text Response");
            if !ai_message.tool_calls.is_empty() {
                info!(step, count = ai_message.tool_calls.len(), "🛠️ LLM requested tool calls");
            } else {
                final_result = Some(ai_message.content);
                break;
            }

            // tool calls
            for tool_call in ai_message.tool_calls.iter_mut() {
                info!(step, tool = %tool_call.name, id = %tool_call.id, args = %tool_call.arguments, "🚀 Executing tool");
                // execute tools
                let control = self.execute_tool(step, tool_call).await.context("execute tool")?;
                let result = extract_agent_control!(control);

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
            self.hooks.on_step_end(step, &ai_message).await.with_context(|| format!("step {step} start"))?;
        }

        match final_result {
            Some(mut final_result) => {
                info!(output = final_result, "🛑 Finish task with final output");
                self.add_memory(query, &mut final_result).await?;
                Ok(AgentControl::Normal(final_result))
            }
            None => {
                warn!("Agent reached max steps without termination");
                self.hooks.on_agent_max_iteration().await.context("max iter hook")?;
                Ok(AgentControl::Normal("Task do not finish yet".to_string()))
            }
        }
    }

    pub async fn chat(&mut self, query: &str) -> AgentResult<AgentControl<String>> {
        info!(query = %query, "🤖 Agent started with user query");
        
        // init context
        let memories = self.search_memory(query).await.context("search memory")?;
        let mut messages = self.build_context(query, memories).await.context("build context")?;
        
        // chat with llm
        let control = self.llm_chat(0, &mut messages, false).await.context("chat with llm")?;
        let mut ai_message = extract_agent_control!(control);

        info!(role = "AI", content = ai_message.content, "🗣️ LLM Text Response");
        self.add_memory(query, &mut ai_message.content).await?;

        Ok(AgentControl::Normal(ai_message.content))
    }

    async fn execute_tool(&mut self, step: usize, tool_call: &mut ToolCall) -> AgentResult<AgentControl<ToolCallResult>> {
        let flow = self.middlewares
            .intercept_tool_call(tool_call)
            .await.context("intercept tool call")?;
        return_middleware_break!(flow);
        
        self.hooks.on_tool_start(step, tool_call).await.context("tool start hook")?;

        let mut result = self.toolbox.execute_tool(&tool_call.name, &tool_call.arguments).await.context("execute tool")?;

        let flow = self.middlewares
            .intercept_tool_result(&tool_call.name, &mut result)
            .await.context("intercept tool result")?;
        return_middleware_break!(flow);
        
        self.hooks.on_tool_end(step, &result).await.context("tool end hook")?;

        if result.is_error {
            self.hooks.on_tool_error(step, &result.context).await.context("tool error hook")?;
        }
        
        Ok(AgentControl::Normal(result))
    }

    async fn add_memory(&mut self, user_input: &str, ai_response: &mut String) -> AgentResult<()> {
        debug!(user_input = user_input, ai_response = ai_response, "add memory");
        
        self.middlewares    
            .intercept_memory_add(user_input, ai_response).await
            .context("intercept memory add")?;

        self.hooks.on_memory_add(user_input, ai_response).await.context("memory add hook")?;

        self.memory
            .add(user_input, ai_response).await
            .map_err(|e| AgentError::Memory(Box::new(e)))
            .context("add new memory")?;

        Ok(())
    }

    async fn search_memory(&mut self, query: &str) -> AgentResult<Vec<ChatMessage>> {
        debug!(query = query, "search memory");
        let memories: Vec<ChatMessage> = self.memory.search(query).await
            .map_err(|e| AgentError::Memory(Box::new(e)))
            .context("search memory")?;
        self.hooks.on_memory_search(query, &memories).await.context("memory search hook")?;

        Ok(memories)
    }

    async fn build_context(&mut self, query: &str, memories: Vec<ChatMessage>) -> AgentResult<Vec<ChatMessage>> {
        debug!("build context");
        let messages: Vec<ChatMessage> = self.context_builder.build(query, memories);
        self.hooks.on_context_build(query, &messages).await.context("context build hook")?;

        Ok(messages)
    }

    async fn llm_chat(&mut self, step: usize, messages: &mut Vec<ChatMessage>, with_tool: bool) -> AgentResult<AgentControl<AssistantMessage>> {
        
        
        self.hooks.on_llm_start(step, messages).await.context("llm start hook")?;

        let mut ai_message = if with_tool {
            self.llm.chat(messages.as_slice()).await?.message
        } else {
            self.llm.chat_no_tools(messages.as_slice()).await?.message
        };

        let flow = self.middlewares
            .intercept_llm_out(&mut ai_message).await
            .context("intercept llm out")?;
        return_middleware_break!(flow);

        self.hooks.on_llm_end(step, &ai_message).await.context("llm end hook")?;

        Ok(AgentControl::Normal(ai_message))
    }
}
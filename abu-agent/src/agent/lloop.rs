use std::fmt::Display;

use abu_base::chat::{AssistantMessage, ChatMessage, ToolCall, ToolDefinition};
use abu_provider::ChatProvide;
use abu_tool::ToolCallResult;
use crate::compact::ContextCompact;
use crate::{middleware::MiddlewareFlow, AgentError};
use crate::memory::Memory;
use super::{Agent, AgentContext, AgentResult};

use thiserrorctx::Context;
use tracing::{debug, info, warn};

pub enum AgentControl<T> {
    Normal(T),
    Break(String),
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

impl<P: ChatProvide, M: Memory, C: ContextCompact> Agent<P, M, C> {
    pub fn tool_list(&self) -> &[ToolDefinition] {
        self.tools.tool_definitions()
    }

    pub async fn run(&mut self, query: &str) -> AgentResult<AgentControl<String>> {
        info!(query = %query, "🤖 Agent started with user query");
        self.hooks.on_agent_start(query).await.context("agent start hook")?;

        // 1. 初始当前论次的 context（System_prompt + Memory + 历史 session ）
        let control = self.build_context(query).await?;
        let mut context = extract_agent_control!(control);

        // 2. agent loop
        let mut final_result = None; 
        for step in 0..self.config.max_iteration {
            // 2.1 compact 检查
            self.compact_context(&mut context).await?;

            // 2.2 调用 llm api
            let control = self.llm_chat(step, &mut context, true).await
                .with_context(|| format!("chat with llm in step {}", step))?;
            let mut ai_message = extract_agent_control!(control);
            context.session.push(ai_message.clone().into());

            info!(step, role = "AI", content = ai_message.content, "🗣️ LLM Text Response");
            if !ai_message.tool_calls.is_empty() {
                info!(step, count = ai_message.tool_calls.len(), "🛠️ LLM requested tool calls");
            } else {
                final_result = Some(ai_message.content);
                break;
            }

            // 2.3 工具调用
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
                context.session.push(ChatMessage::tool(tool_content, tool_call.id.clone()));
            }

            debug!(step, "🔄 Agent step end");
            self.hooks.on_step_end(step, &ai_message).await.with_context(|| format!("step {step} start"))?;
        }

        // 3. 保存 memory + session
        self.session = context.session;
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

    async fn build_context(&mut self, query: &str) -> AgentResult<AgentControl<AgentContext>> {
        // 1. 构造 system prompt
        let mut system_prompt = self.system_prompt.clone();
        let flow = self.middlewares
            .intercept_system_prompt(&mut system_prompt).await
            .context("intercept system prompt")?;
        return_middleware_break!(flow);

        // 2. 加载记忆
        let memory: Vec<ChatMessage> = self.memory.search(query).await
            .map_err(|e| AgentError::Memory(e.to_string()))
            .context("search memory")?;
        self.hooks.on_memory_search(query, &memory).await.context("memory search hook")?;

        // 3. 组装
        let context = AgentContext {
            system_prompt,
            memory,
            session: self.session.clone(),
        };

        Ok(AgentControl::Normal(context))
    }

    async fn compact_context(&mut self, context: &mut AgentContext) -> AgentResult<()> {
        self.compact
            .compact(context).await
            .map_err(|e| AgentError::Compact(e.to_string()))?;
        Ok(())
    } 

    async fn llm_chat(&mut self, step: usize, context: &mut AgentContext, with_tool: bool) -> AgentResult<AgentControl<AssistantMessage>> {
        let flow = self.middlewares
            .intercept_llm_input(&mut context.session).await
            .context("intercept llm input")?;
        return_middleware_break!(flow);
        
        let messages = context.assemble_messages();
        self.hooks.on_llm_start(step, &messages).await.context("llm start hook")?;

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

    async fn execute_tool(&mut self, step: usize, tool_call: &mut ToolCall) -> AgentResult<AgentControl<ToolCallResult>> {
        let flow = self.middlewares
            .intercept_tool_call(tool_call)
            .await.context("intercept tool call")?;
        return_middleware_break!(flow);
        
        self.hooks.on_tool_start(step, tool_call).await.context("tool start hook")?;

        let mut result = self.tools.execute_tool(&tool_call.name, &tool_call.arguments).await.context("execute tool")?;

        let flow = self.middlewares
            .intercept_tool_result(&tool_call, &mut result)
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
            .map_err(|e| AgentError::Memory(e.to_string()))
            .context("add new memory")?;

        Ok(())
    }
}

impl<T> AgentControl<T> {
    pub fn unwrap(self) -> T {
        match self {
            Self::Break(_) => panic!("control is break!"),
            Self::Normal(v) => v,
        }
    }
}

impl<T: Display> Display for AgentControl<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Break(s) => write!(f, "agent flow break: {}", s),
            Self::Normal(r) => write!(f, "{}", r),
        }
    }
}
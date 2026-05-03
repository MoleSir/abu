use abu_base::chat::ChatMessage;
use abu_provider::ChatProvide;
use tracing::debug;
use crate::{model::ChatModel, AgentError};

use super::Memory;

pub struct AugmentedMemory<P: ChatProvide> {
    llm: ChatModel<P>,
    /// 存储提炼出的长期事实
    memory_tokens: Vec<String>,
}

impl<P: ChatProvide> AugmentedMemory<P> {
    pub fn new(llm: ChatModel<P>) -> Self {
        Self { 
            llm,
            memory_tokens: vec![] 
        }
    }
}

#[async_trait::async_trait]
impl<P: ChatProvide> Memory for AugmentedMemory<P> {
    type Error = AgentError; // 假设使用你定义的 AgentError

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error> {
        // 1. 构造提炼 Prompt
        let fact_extraction_prompt = format!(
            "Analyze the following conversation turn and extract any CORE facts, user preferences, or project-specific decisions.\n\n\
             Guidelines:\n\
             - Extract user preferences (e.g., 'prefers Python over Rust')\n\
             - Extract project facts (e.g., 'API endpoint is v1/auth')\n\
             - If no new important information is found, respond ONLY with 'NONE'.\n\n\
             Conversation Turn:\nUser: {user_input}\nAI: {ai_response}\n\n\
             Concise Fact:"
        );

        // 2. 调用 LLM 进行提炼
        let messages = vec![
            ChatMessage::system("You are a knowledge extraction assistant. You only output concise facts or 'NONE'."),
            ChatMessage::user(fact_extraction_prompt),
        ];

        // 这里假设 ChatModel 有直接 chat 的方法
        let response = self.llm
            .chat_no_tools(&messages).await
            .map_err(|e| AgentError::ChatProvider(e.to_string()))?
            .message;

        let content = response.content.trim();

        // 3. 如果有新事实，则保存
        if !content.is_empty() && content.to_uppercase() != "NONE" {
            debug!(fact = %content, "🧠 New long-term memory extracted");
            self.memory_tokens.push(content.to_string());
        }

        Ok(())
    }
    
    async fn search(&self, _query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        // 如果没有记忆，直接返回空
        if self.memory_tokens.is_empty() {
            return Ok(vec![]);
        }

        // 将所有事实汇总成一条系统消息或者一条带有背景知识的用户消息
        // 建议作为一条 System 消息的一部分，告知 Agent 之前记住的事实
        let mut facts_block = String::from("### Known Facts & User Preferences:\n");
        for (i, fact) in self.memory_tokens.iter().enumerate() {
            facts_block.push_str(&format!("{}. {}\n", i + 1, fact));
        }

        // 包装成 ChatMessage
        // 这里推荐使用 System 角色，这样它会被 AgentContext 组装到 System Prompt 之后
        Ok(vec![
            ChatMessage::system(facts_block)
        ])
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.memory_tokens.clear();
        Ok(())
    }
}
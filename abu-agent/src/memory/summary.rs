use std::sync::Arc;
use abu_api::chat::ChatMessage;
use tracing::debug;
use crate::{llm::LLM, AgentCtxError, AgentResult};
use super::Memory;

pub struct SummarizationMemory {
    llm: Arc<LLM>,
    messages: Vec<ChatMessage>,
    summary_threshold: usize,
}

impl SummarizationMemory {
    pub fn from_env(summary_threshold: usize) -> AgentResult<Self> {
        let llm = LLM::from_env()?;
        Ok(Self::new(Arc::new(llm), summary_threshold))
    }

    pub fn new(llm: Arc<LLM>, summary_threshold: usize) -> Self {
        Self { 
            llm,
            messages: vec![], 
            summary_threshold,
        }
    }

    pub fn user_message_count(&self) -> usize {
        self.messages.iter()
            .filter(|m| m.is_user())
            .count()
    }

    /// call llm to summary `messages` and reset `messages`
    async fn consolidate_memory(&mut self) -> AgentResult<()> {
        debug!("--- [Memory Consolidation Triggered] ---");

        // collection all messages
        let buffer_text = self.messages.iter()
            .map(|m| m.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        // send to llm
        let summarization_prompt = format!(
           "Summarize this conversation for continuity. Include:  \
            1) What was accomplished, 2) Current state, 3) Key decisions made. \
            Be concise but preserve critical details.\n\n{}",
            buffer_text
        );
        let messages = vec![
            ChatMessage::system("You are an expert summarization engine."),
            ChatMessage::user(summarization_prompt),
        ];
        let response = self.llm.chat(&messages, &[], 0.7).await?;
        
        // update messages
        self.messages.clear();
        self.messages.push(ChatMessage::user(format!("[Conversation compressed]: {}", response.content)));
        self.messages.push(ChatMessage::assistant("Understood. I have the context from the summary. Continuing.", []));

        Ok(())
    }
}

#[async_trait::async_trait]
impl Memory for SummarizationMemory {
    type Error = AgentCtxError;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error> {
        self.messages.push(ChatMessage::user(user_input));
        self.messages.push(ChatMessage::assistant(ai_response, []));
        if self.user_message_count() >= self.summary_threshold {
            self.consolidate_memory().await?;
        }
        Ok(())
    }

    async fn search(&mut self, _query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        Ok(self.messages.clone())
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.messages.clear();
        Ok(())
    }
}

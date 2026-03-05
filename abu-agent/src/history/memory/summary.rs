use std::sync::Arc;
use abu_api::chat::ChatMessage;
use tracing::debug;
use crate::llm::LLM;
use super::Memory;

pub struct SummarizationMemory {
    llm: Arc<LLM>,
    messages: Vec<ChatMessage>,
    summary_threshold: usize,
}

impl SummarizationMemory {
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
    async fn consolidate_memory(&mut self) -> anyhow::Result<()> {
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
        let response = self.llm.chat(&messages, &[], 0.7).await.map_err(|e| anyhow::anyhow!(e.to_string()))?;
        
        // update messages
        self.messages.clear();
        self.messages.push(ChatMessage::user(format!("[Conversation compressed]: {}", response.content)));
        self.messages.push(ChatMessage::assistant("Understood. I have the context from the summary. Continuing.", []));

        Ok(())
    }
}

#[async_trait::async_trait]
impl Memory for SummarizationMemory {
    async fn fork(&self) -> anyhow::Result<Box<dyn Memory>> {
        Ok(Box::new(Self::new(self.llm.clone(), self.summary_threshold)))
    }

    async fn add_message(&mut self, message: ChatMessage) -> anyhow::Result<()> {
        self.messages.push(message);
        Ok(())
    }

    async fn compact_messages(&mut self, _query: &str) -> anyhow::Result<Vec<ChatMessage>> {
        if self.user_message_count() >= self.summary_threshold {
            self.consolidate_memory().await?;
        }
        Ok(self.messages.clone())
    }

    async fn clear(&mut self) -> anyhow::Result<()> {
        self.messages.clear();
        Ok(())
    }
}

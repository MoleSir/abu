pub mod memory;
use abu_api::chat::ChatMessage;
use memory::Memory;
use crate::{AgentError, AgentResult};

pub struct AgentHistory {
    pub memory: Box<dyn Memory>,
    pub system_prompt: String,
    pub messages: Vec<ChatMessage>,
}

impl AgentHistory {
    pub fn new(memory: Box<dyn Memory>, system_prompt: impl Into<String>) -> Self {
        Self {
            memory,
            system_prompt: system_prompt.into(),
            messages: vec![],
        }
    }

    pub async fn fork(&self, system_prompt: impl Into<String>) -> AgentResult<Self> {
        Ok(Self {
            memory: self.memory.fork().await.map_err(AgentError::Memory)?,
            system_prompt: system_prompt.into(),
            messages: vec![],
        })
    }

    #[inline]
    pub async fn add_message(&mut self, message: ChatMessage) -> AgentResult<()> {
        self.messages.push(message.clone());
        self.memory.add_message(message).await.map_err(AgentError::Memory)?;
        Ok(())
    }

    #[inline]
    pub async fn clear(&mut self) -> AgentResult<()> {
        self.messages.clear();
        self.memory.clear().await.map_err(AgentError::Memory)?;
        Ok(())
    }

    pub async fn compact(&mut self, query: &str) -> AgentResult<()> {
        self.messages = vec![ChatMessage::system(self.system_prompt.clone())];
        self.messages.extend(self.memory.compact_messages(query).await.map_err(AgentError::Memory)?);
        Ok(())
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }
}
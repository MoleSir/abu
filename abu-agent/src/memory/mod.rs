mod sequential;
mod slidingwindow;
mod summary;

pub use sequential::Sequential;
pub use slidingwindow::SliceWindow;
use abu_api::chat::ChatMessage;

#[async_trait::async_trait]
pub trait MemoryStrategy {
    async fn add_message(&mut self, message: ChatMessage) -> MemoryResult<()>;
    async fn compact_messages(&mut self, query: &str) -> MemoryResult<Vec<ChatMessage>>;
    async fn clear(&mut self) -> MemoryResult<()>;
}

pub struct Memory {
    pub strategy: Box<dyn MemoryStrategy>,
    pub system_prompt: String,
}

impl Memory {
    pub fn new(strategy: Box<dyn MemoryStrategy>, system_prompt: impl Into<String>) -> Self {
        Self {
            strategy,
            system_prompt: system_prompt.into(),
        }
    }

    #[inline]
    pub async fn add_message(&mut self, message: ChatMessage) -> MemoryResult<()> {
        self.strategy.add_message(message).await
    }

    #[inline]
    pub async fn clear(&mut self) -> MemoryResult<()> {
        self.strategy.clear().await
    }

    pub async fn load_messages(&mut self, query: &str) -> MemoryResult<Vec<ChatMessage>> {
        let mut messages  = vec![ChatMessage::system(self.system_prompt.clone())];
        messages.extend(self.strategy.compact_messages(query).await?);
        Ok(messages)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryError {

}

pub type MemoryResult<T> = std::result::Result<T, MemoryError>;

mod sequential;
mod slidingwindow;
mod summary;

use abu_api::chat::ChatMessage;
pub use sequential::SequentialMemory;
pub use slidingwindow::SliceWindowMemory;
pub use summary::SummarizationMemory;

// #[async_trait::async_trait]
// pub trait Memory : Send + Sync {
//     async fn fork(&self) -> anyhow::Result<Box<dyn Memory>>;
//     async fn add_message(&mut self, message: ChatMessage) -> anyhow::Result<()>;
//     async fn compact_messages(&mut self, query: &str) -> anyhow::Result<Vec<ChatMessage>>;
//     async fn clear(&mut self) -> anyhow::Result<()>;
// }

#[async_trait::async_trait]
pub trait Memory : Send + Sync {
    type Error: std::error::Error + 'static;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error>;
    async fn search(&mut self, query: &str) -> Result<Vec<ChatMessage>, Self::Error>;
    async fn clear(&mut self) -> Result<(), Self::Error>;
}
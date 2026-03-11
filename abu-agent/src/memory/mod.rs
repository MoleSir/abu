mod sequential;
mod slidingwindow;
mod summary;
mod retrieval;

use abu_base::chat::ChatMessage;
pub use sequential::SequentialMemory;
pub use slidingwindow::SliceWindowMemory;
pub use summary::SummarizationMemory;
pub use retrieval::RetrievalMemory;

#[allow(async_fn_in_trait)]
pub trait Memory : Send + Sync {
    type Error: std::error::Error + 'static;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error>;
    async fn search(&mut self, query: &str) -> Result<Vec<ChatMessage>, Self::Error>;
    async fn clear(&mut self) -> Result<(), Self::Error>;
}
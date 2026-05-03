mod retrieval;
mod augmented;

use std::{convert::Infallible, fmt::Display};

use abu_base::chat::ChatMessage;
pub use retrieval::RetrievalMemory;
pub use augmented::AugmentedMemory;

#[async_trait::async_trait]
pub trait Memory : Send + Sync {
    type Error: Display + 'static + Send + Sync;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error>;
    async fn search(&self, query: &str) -> Result<Vec<ChatMessage>, Self::Error>;
    async fn clear(&mut self) -> Result<(), Self::Error>;
}

pub struct NoMemory;
#[async_trait::async_trait]
impl Memory for NoMemory {
    type Error = Infallible;
    async fn add(&mut self, _user_input: &str, _ai_response: &str) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn search(&self, _query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        Ok(vec![])
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
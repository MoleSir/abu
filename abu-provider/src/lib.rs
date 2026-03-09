pub mod openai;
pub mod deepseek;
pub mod anthropic;
mod error;
pub use error::*;

use abu_base::{
    chat::{ChatRequest, ChatResponse}, 
    embed::{EmbedRequest, EmbedResponse},
};

#[async_trait::async_trait]
pub trait ChatProvide : Send + Sync {
    type Error: std::error::Error + 'static + Sync + Send;
    async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, Self::Error>;
}

#[async_trait::async_trait]
pub trait EmbedProvide : Send + Sync {
    type Error: std::error::Error + 'static + Sync + Send;
    async fn embed(&self, request: &EmbedRequest) -> Result<EmbedResponse, Self::Error>;
}

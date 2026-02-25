pub mod chat;
pub mod image;

use crate::{
    chat::{ChatRequest, ChatResponse}, 
    image::{ImageRequest, ImageResponse}
};
use std::time::Duration;
use chat::ChatRequestBuilderError;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

const TIMEOUT: u64 = 60;

#[async_trait]
pub trait ApiRequest: Serialize {
    type Response: DeserializeOwned;
    async fn send(&self, url: &str, api_key: &str) -> ApiResult<Self::Response> {
        // Build response
        let res = reqwest::Client::new()
            .post(url)
            .json(&self)        
            .bearer_auth(api_key)
            .timeout(Duration::from_secs(TIMEOUT))
            .send().await?
            .json::<Self::Response>()
            .await?;
        Ok(res)
    }

    fn send_sync(&self, url: &str, api_key: &str) -> ApiResult<Self::Response> {
        let res = reqwest::blocking::Client::new()
            .post(url)
            .json(&self)        
            .bearer_auth(api_key)
            .timeout(Duration::from_secs(TIMEOUT))
            .send()?
            .json::<Self::Response>()?;
        Ok(res)
    }
}

pub struct Client {
    base_url: String,
    api_key: String,
}

impl Client {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key: api_key.into()
        }
    }

    pub fn from_env() -> ApiResult<Self> {
        dotenv::dotenv()?;
        let base_url = std::env::var("BASE_URL")?;
        let api_key = std::env::var("OPENAI_API_KEY")?;
        Ok(Self { base_url, api_key })
    }

    pub async fn chat(&self, request: &ChatRequest) -> ApiResult<ChatResponse> {
        let url = format!("{}/chat/completions", self.base_url);
        request.send(&url, &self.api_key).await
    }

    pub fn chat_sync(&self, request: &ChatRequest) -> ApiResult<ChatResponse> {
        let url = format!("{}/chat/completions", self.base_url);
        request.send_sync(&url, &self.api_key)
    }

    pub async fn image(&self, request: &ImageRequest) -> ApiResult<ImageResponse> {
        let url = format!("{}/images/generations", self.base_url);
        request.send(&url, &self.api_key).await
    }

    pub async fn image_sync(&self, request: &ImageRequest) -> ApiResult<ImageResponse> {
        let url = format!("{}/images/generations", self.base_url);
        request.send_sync(&url, &self.api_key)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error(transparent)]
    EnvVar(#[from] std::env::VarError),

    #[error(transparent)]
    Dotenv(#[from] dotenv::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    ChatRequest(#[from] ChatRequestBuilderError),
    
    #[error("Except messgae {0}")]
    ExceptMessage(&'static str),
}

pub type ApiResult<T> = std::result::Result<T, ApiError>;

#[cfg(test)]
mod test {
    use crate::{chat::{ChatMessage, ChatRequestBuilder}, Client};

    #[test]
    fn test_chat1() {
        let client = Client::from_env().expect("new client");
        let request = ChatRequestBuilder::default()
            .model(std::env::var("MODEL_ID").expect("No MODEL_ID"))
            .messages([
                ChatMessage::user("hi!"),
            ])
            .build()
            .expect("build request");
                
        let response = client.chat_sync(&request).expect("chat");

        println!("{:#?}", response);
    }
}
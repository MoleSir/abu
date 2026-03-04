pub mod chat;
pub mod image;

use std::time::Duration;
use chat::ChatRequestBuilderError;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

const TIMEOUT: u64 = 60;

#[derive(Clone)]
pub struct Credentials {
    base_url: String,
    api_key: String,
}

#[async_trait]
pub trait ApiRequest: Serialize {
    type Response: DeserializeOwned;
    fn url(base_url: &str) -> String;
    async fn send(&self, credentials: &Credentials) -> ApiResult<Self::Response> {
        let url = Self::url(&credentials.base_url);
        let res = reqwest::Client::new()
            .post(url)
            .json(&self)        
            .bearer_auth(&credentials.api_key)
            .timeout(Duration::from_secs(TIMEOUT))
            .send().await?
            .json::<Self::Response>()
            .await?;
        Ok(res)
    }
}

impl Credentials {
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
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

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
    use crate::{chat::{ChatMessage, ChatRequestBuilder}, ApiRequest, Credentials};

    #[tokio::test]
    async fn test_chat1() {
        let credentials = Credentials::from_env().expect("new client");
        let request = ChatRequestBuilder::default()
            .model(std::env::var("MODEL_ID").expect("No MODEL_ID"))
            .messages([
                ChatMessage::user("hi!"),
            ])
            .build()
            .expect("build request");
                
        let response =  request.send(&credentials).await.expect("chat");

        println!("{:#?}", response);
    }
}
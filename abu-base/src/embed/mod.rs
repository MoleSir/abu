use serde::{Deserialize, Serialize};
use super::common::Usage;

#[derive(Debug, Clone, Serialize)]
pub struct EmbedRequest {
    pub input: Vec<String>,
    pub model: String,
}

impl EmbedRequest {
    pub fn new(input: impl Into<Vec<String>>, model: impl Into<String>) -> Self {
        Self { input: input.into(), model: model.into() }
    }

    pub fn single(input: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            input: vec![input.into()],
            model: model.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResponse {
    pub embeddings: Vec<Vec<f32>>,
    pub usage: Usage,
}
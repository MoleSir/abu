use abu_provider::{embed::EmbedRequest, EmbedProvide};
use crate::document::Chunk;

pub struct Embedder<P> {
    provider: P,
    model: String,
}

pub struct EmbeddedChunk {
    pub chunk: Chunk,
    pub embedding: Vec<f32>,
}

impl<P: EmbedProvide> Embedder<P> {
    pub fn new(provider: P, model: impl Into<String>) -> Self {
        Self {
            provider,
            model: model.into(),
        }
    }

    pub async fn embed_chunks(&self, chunks: Vec<Chunk>) -> Result<Vec<EmbeddedChunk>, EmbedError> {
        let texts = chunks.iter().map(|c| c.text.clone()).collect();
        let embeddings = self.embed_texts(texts).await?;
        let result = chunks
            .into_iter()
            .zip(embeddings)
            .map(|(chunk, embedding)| EmbeddedChunk { chunk, embedding })
            .collect();
        Ok(result)
    }

    pub async fn embed_texts(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError> {
        let request = EmbedRequest { input: texts ,model: self.model.clone() };
        let response = self.provider
            .embed(&request).await
            .map_err(|e| EmbedError::EmbedProvide(Box::new(e)))?;
        Ok(response.embeddings)
    }

    pub async fn embed_text(&self, text: impl Into<String>) -> Result<Vec<f32>, EmbedError> {
        let request = EmbedRequest { input: vec![text.into()] ,model: self.model.clone() };
        let response = self.provider
            .embed(&request).await
            .map_err(|e| EmbedError::EmbedProvide(Box::new(e)))?;
        
        let embedding = response.embeddings.into_iter()
            .next()
            .ok_or_else(|| EmbedError::NoEmbedding)?;
        Ok(embedding)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("load error: {0}")]
    EmbedProvide(Box<dyn std::error::Error + 'static + Send + Sync>),

    #[error("empty embedding result")]
    NoEmbedding,
}
use abu_base::chat::ChatMessage;
use abu_provider::EmbedProvide;
use abu_rag::{embed::{EmbedError, Embedder}, vectordb::{FlatL2Index, InMemoryStorage, VectorDB, VectorDBError, VectorId}};
use super::Memory;

pub struct RetrievalMemory<P> {
    embedder: Embedder<P>,
    db: VectorDB<FlatL2Index<f32>, InMemoryStorage<String>>,
    top_k: usize,
}

impl<P: EmbedProvide> RetrievalMemory<P> {
    pub fn new(provider: P, model: impl Into<String>, top_k: usize) -> Self {
        Self { 
            top_k, 
            embedder: Embedder::new(provider, model), 
            db: VectorDB::new(FlatL2Index::new(), InMemoryStorage::new()) 
        }
    }

    fn new_id(&self) -> VectorId {
        self.db.len() as VectorId
    }
}

impl<P: EmbedProvide> Memory for RetrievalMemory<P> {
    type Error = RetrievalMemoryError;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error> {
        let content = format!("User: {}\nAI: {}", user_input, ai_response);
        let embedding = self.embedder.embed_text(content.clone()).await?;
        self.db.add(self.new_id(), embedding, content).await?;
        Ok(())
    }
    
    async fn search(&self, query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        let query_embedding = self.embedder.embed_text(query).await?;
        let result = self.db.search(&query_embedding, self.top_k).await?;
        let retrieved_docs = result.iter()
            .map(|(_, s)| s.as_ref().as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        let message = ChatMessage::user(format!("### Relevant Information Retrieved from Memory:\n{}", retrieved_docs));
        Ok(vec![message])
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.db.clear().await?;
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RetrievalMemoryError {
    #[error(transparent)]
    Embed(#[from] EmbedError),

    #[error(transparent)]
    VectorDB(#[from] VectorDBError)
}
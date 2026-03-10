use std::convert::Infallible;
use crate::{document::Chunk, embed::EmbeddedChunk};
use super::{util, ScoredChunk, VectorDB};

pub struct MemoryVectorStore {
    collection: Vec<EmbeddedChunk>,
}

impl MemoryVectorStore {
    pub fn new() -> Self {
        Self { collection: Vec::new() }
    }
}

impl Default for MemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorDB for MemoryVectorStore {
    type Error = Infallible;

    async fn add_chunks<I>(&mut self, chunks: I) -> Result<(), Self::Error> 
    where 
        I: IntoIterator<Item = EmbeddedChunk>
    {
        self.collection.extend(chunks);
        Ok(())
    }

    async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<ScoredChunk>, Self::Error> {
        let mut scored_chunks: Vec<(&Chunk, f32)> = self.collection
            .iter()
            .map(|ec| (
                &ec.chunk, util::cosine_similarity(&ec.embedding, query_embedding)
            ))
            .collect();

        scored_chunks.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        scored_chunks.truncate(limit);
        
        Ok(
            scored_chunks
                .into_iter()
                .map(|(c, s)| ScoredChunk::new(c.clone(), s))
                .collect()
        )
    }
}

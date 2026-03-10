#[cfg(feature = "sqlite")]
pub mod sqlite;

mod memory;
pub use memory::*;
pub mod util;

use crate::{document::Chunk, embed::EmbeddedChunk};

#[derive(Debug, Clone)]
pub struct ScoredChunk {
    pub chunk: Chunk,
    pub score: f32,
}

#[allow(async_fn_in_trait)]
pub trait VectorDB {
    type Error: std::error::Error + 'static + Send + Sync;

    async fn add_chunks<I>(&mut self, chunks: I) -> Result<(), Self::Error> 
    where 
        I: IntoIterator<Item = EmbeddedChunk>;

    async fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<ScoredChunk>, Self::Error>;
}

impl ScoredChunk {
    pub fn new(chunk: Chunk, score: f32) -> Self {
        Self { chunk, score }
    }
}
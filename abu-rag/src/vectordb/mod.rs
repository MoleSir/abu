mod index;
mod storage;
mod metric;
mod vector;
pub use index::*;
pub use vector::*;
pub use storage::*;
pub use metric::*;
use std::borrow::Cow;

pub type VectorId = u64;

pub struct VectorDB<I, S> {
    index: I,
    storage: S,
}

impl<I: VectorIndex, S: VectorStorage> VectorDB<I, S> {
    pub fn new(index: I, storage: S) -> Self {
        Self { index, storage }
    }
}

impl<I: VectorIndex, S: VectorStorage> VectorDB<I, S> {
    pub async fn add(&mut self, id: VectorId, vector: Vec<I::F>, payload: S::Payload) -> Result<(), VectorDBError> {        
        self.index.add(id, vector).await.map_err(|e| VectorDBError::Index(Box::new(e)))?;
        self.storage.add(id, payload).await.map_err(|e| VectorDBError::Index(Box::new(e)))?;
        Ok(())
    }

    pub async fn search(&self, query: &[I::F], top_k: usize) -> Result<Vec<(ScoredId<I::F>, Cow<S::Payload>)>, VectorDBError> {
        let scored_ids = self.index
            .search(query, top_k).await
            .map_err(|e| VectorDBError::Index(Box::new(e)))?;
        
        let mut results = Vec::with_capacity(scored_ids.len());
        for scored_id in scored_ids {
            if let Some(record) = self.storage.get(scored_id.id).await.map_err(|e| VectorDBError::Storage(Box::new(e)))? {
                results.push((scored_id, record));
            }
        }
        Ok(results)
    }

    pub async fn clear(&mut self) -> Result<(), VectorDBError> {
        self.index.clear().await.map_err(|e| VectorDBError::Index(Box::new(e)))?;
        self.storage.clear().await.map_err(|e| VectorDBError::Index(Box::new(e)))?;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VectorDBError {
    #[error("index error: {0}")]
    Index(Box<dyn std::error::Error + 'static + Send + Sync>),

    #[error("storage error: {0}")]
    Storage(Box<dyn std::error::Error + 'static + Send + Sync>),
}
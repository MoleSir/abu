use std::{collections::HashMap, marker::PhantomData};
use crate::vectordb::{Cosine, Float, VectorId, VectorMetric, VectorMetricError, L2};
use super::{ScoredId, VectorIndex};

pub struct FlatIndex<M: VectorMetric, F: Float = f32> {
    vectors: HashMap<VectorId, Vec<F>>,
    _marker: PhantomData<M>,
}

pub type FlatL2Index<F> = FlatIndex<L2, F>;
pub type FlatCosineIndex<F> = FlatIndex<Cosine, F>;

impl<M: VectorMetric, F: Float> FlatIndex<M, F> {
    pub fn new() -> Self {
        Self { vectors: HashMap::new(), _marker: PhantomData }
    }
}

impl<M: VectorMetric, F: Float> VectorIndex for FlatIndex<M, F> {
    type F = F;
    type Error = VectorMetricError;

    async fn add(&mut self, id: VectorId, vector: Vec<F>) -> Result<(), Self::Error> {
        self.vectors.insert(id, vector);
        Ok(())
    }

    async fn delete(&mut self, id: VectorId) -> Result<(), Self::Error> {
        self.vectors.remove(&id);
        Ok(())
    }

    async fn search(&self, query: &[F], top_k: usize) -> Result<Vec<ScoredId<F>>, Self::Error> {
        let mut scored_vectors = vec![];
        for (id, vector) in self.vectors.iter() {
            let score = M::score(query, &vector)?;
            scored_vectors.push(ScoredId::new(id.clone(), score));
        }

        scored_vectors.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        scored_vectors.truncate(top_k);
        Ok(scored_vectors)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.vectors.clear();
        Ok(())
    }

    fn len(&self) -> usize {
        self.vectors.len()
    }
}
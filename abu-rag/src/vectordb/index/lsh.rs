use std::{collections::HashMap, marker::PhantomData};
use rand::{distr::uniform::SampleUniform, RngExt};
use crate::vectordb::{Dot, Float, VectorId, VectorMetric, VectorMetricError};
use super::{ScoredId, VectorIndex};

/// Random Hyperplane LSH
pub struct LshIndex<M: VectorMetric, F: Float> {
    #[allow(unused)]
    dim: usize,

    // random hyperplanes
    hyperplanes: Vec<Vec<F>>,

    // bucket -> vectors ids
    buckets: HashMap<u64, Vec<VectorId>>,

    // vectos
    vectors: HashMap<VectorId, Vec<F>>,

    _marker: PhantomData<M>,
}   

impl<M: VectorMetric, F: Float + SampleUniform> LshIndex<M, F> {
    pub fn new(bucket_bits: usize, dim: usize) -> Self {
        Self {
            dim,
            hyperplanes: Self::random_planes(bucket_bits, dim),
            buckets: HashMap::new(),
            vectors: HashMap::new(),
            _marker: PhantomData,
        }
    }

    fn random_planes(bucket_bits: usize, dim: usize) -> Vec<Vec<F>> {
        let mut rng = rand::rng();
        (0..bucket_bits)
            .map(|_| {
                (0..dim)
                    .map(|_| rng.random_range(-F::one()..F::one()))
                    .collect()
            })
            .collect()
    }

    fn hash(&self, vector: &[F]) -> Result<u64, VectorMetricError> {
        // TODO: overflow
        let mut hash_value = 0;
        for (i, plane) in self.hyperplanes.iter().enumerate() {
            let dot = Dot::score(vector, plane)?;
            if dot > F::zero() {
                hash_value |= 1 << i;
            }
        }
        Ok(hash_value)
    }
}

impl<M: VectorMetric, F: Float + SampleUniform> VectorIndex for LshIndex<M, F> {
    type F = F;
    type Error = VectorMetricError;

    async fn add(&mut self, id: VectorId, vector: Vec<F>) -> Result<(), Self::Error> {
        // insert into one bucket!
        let hash = self.hash(&vector)?;
        self.buckets.entry(hash).or_default().push(id.clone());

        // save to vectors
        self.vectors.insert(id, vector);
        Ok(())
    }

    async fn delete(&mut self, id: VectorId) -> Result<(), Self::Error> {
        if let Some(vector) = self.vectors.remove(&id) {
            let hash = self.hash(&vector)?;
            let ids = self.buckets.get_mut(&hash).expect("buckets must have hash!");
            if let Some(pos) = ids.iter().position(|i| *i == id) {
                ids.swap_remove(pos);
            }
            Ok(())
        } else {
            Ok(())
        }
    }

    async fn search(&self, query: &[F], top_k: usize) -> Result<Vec<ScoredId<F>>, Self::Error> {
        let hash = self.hash(&query)?;
        let mut scored_vectors = vec![];

        if let Some(ids) = self.buckets.get(&hash) {
            for id in ids {
                let vector = self.vectors.get(id).expect("must has id");
                let score = M::score(query, vector)?;
                scored_vectors.push(ScoredId::new(id.clone(), score));
            }
        }

        scored_vectors.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
        });
        scored_vectors.truncate(top_k);
        Ok(scored_vectors)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.vectors.clear();
        self.buckets.clear();
        Ok(())
    }

    fn len(&self) -> usize {
        self.vectors.len()
    }
}
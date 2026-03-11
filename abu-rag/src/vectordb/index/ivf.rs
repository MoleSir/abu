use std::{collections::HashMap, marker::PhantomData};
use rand::seq::IndexedRandom;
use crate::vectordb::{Float, VectorId, VectorMetric, VectorMetricError};
use super::{ScoredId, VectorIndex};

pub struct IvfIndex<M: VectorMetric, F: Float> {
    vector_dim: usize,

    // number of clusters
    n_clusters: usize,

    // number of clusters searched
    n_cluster_search: usize,

    // cluster centers
    centroids: Vec<Vec<F>>,

    // inverted lists
    lists: Vec<Vec<VectorId>>,

    // raw vectors
    vectors: HashMap<VectorId, Vec<F>>,

    _marker: PhantomData<M>,
}

impl<M: VectorMetric, F: Float> IvfIndex<M, F> {
    pub fn new(vector_dim: usize, n_clusters: usize, n_cluster_search: usize) -> Self {
        Self {
            vector_dim, n_cluster_search, n_clusters,
            centroids: vec![],
            lists: vec![vec![]; n_clusters],
            vectors: HashMap::new(),
            _marker: PhantomData
        }
    }

    fn nearest_centroid(&self, vector: &[F]) -> Result<(usize, F), VectorMetricError> {
        let mut best = 0;
        let mut best_score = F::min_value();
        for (i, centroid) in self.centroids.iter().enumerate() {
            let score = M::score(vector, &centroid)?;
            if score > best_score {
                best_score = score;
                best = i;
            }
        }
        Ok((best, best_score))
    }

    fn nearest_centroids(&self, vector: &[F]) -> Result<Vec<(usize, F)>, VectorMetricError> {
        let mut scores = Vec::new();

        for (i, centroid) in self.centroids.iter().enumerate() {
            let score = M::score(vector, &centroid)?;
            scores.push((i, score));
        }

        scores.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap()
        });

        scores.truncate(self.n_cluster_search);
        Ok(scores)
    }

    pub fn train(&mut self, samples: &[Vec<F>], iterations: usize) -> Result<(), VectorMetricError> {
        let mut rng = rand::rng();
    
        // init centroids
        self.centroids = samples
            .sample(&mut rng, self.n_clusters)
            .cloned()
            .collect();
    
        for _ in 0..iterations {
            let mut clusters: Vec<Vec<&Vec<F>>> = vec![Vec::new(); self.n_clusters];

            // assign
            for v in samples {
                let mut best = 0;
                let mut best_score = F::min_value();
    
                for (i, c) in self.centroids.iter().enumerate() {
                    let score = M::score(v, c)?;
                    if score > best_score {
                        best = i;
                        best_score = score;
                    }
                }
    
                clusters[best].push(v);
            }
    
            // recompute centroid
            for i in 0..self.n_clusters {
                if clusters[i].is_empty() {
                    continue;
                }
    
                let mut centroid = vec![F::zero(); self.vector_dim];
                for v in &clusters[i] {
    
                    for j in 0..self.vector_dim {
                        centroid[j] += v[j];
                    }
                }
    
                for j in 0..self.vector_dim {
                    centroid[j] /= F::from(clusters[i].len()).expect("from usize");
                }
    
                self.centroids[i] = centroid;
            }
        }

        Ok(())
    }
}

impl<M: VectorMetric, F: Float> VectorIndex for IvfIndex<M, F> {
    type F = F;
    type Error = VectorMetricError;

    async fn add(&mut self, id: VectorId, vector: Vec<F>) -> Result<(), Self::Error> {
        if self.centroids.is_empty() {
            // not yet train
            self.vectors.insert(id, vector);
            return Ok(())
        }

        // get nearest centroid
        let nearest_c_index = self.nearest_centroid(&vector)?.0;
        assert!(nearest_c_index < self.lists.len());

        self.lists[nearest_c_index].push(id.clone());
        self.vectors.insert(id, vector);
        Ok(())
    }

    async fn delete(&mut self, id: VectorId) -> Result<(), Self::Error> {
        if self.centroids.is_empty() {
            // not yet train
            self.vectors.remove(&id);
            Ok(())
        } else {
            if let Some(vector) = self.vectors.remove(&id) {
                let nearest_c_index = self.nearest_centroid(&vector)?.0;
                assert!(nearest_c_index < self.lists.len());
                let ids = self.lists.get_mut(nearest_c_index).expect("must has valid index");
                if let Some(id_index) = ids.iter().position(|i| *i == id) {
                    ids.swap_remove(id_index);
                }
            }
            Ok(())
        }
    }

    async fn search(&self, query: &[F], top_k: usize) -> Result<Vec<ScoredId<F>>, Self::Error> {
        // not yet train
        let mut scored_vectors = if self.centroids.is_empty() {
            let mut scored_vectors = vec![];
            for (id, vector) in self.vectors.iter() {
                let score = M::score(query, &vector)?;
                scored_vectors.push(ScoredId::new(id.clone(), score));
            }
            scored_vectors
        } else {
            // just seach nearest centroids
            let clusters = self.nearest_centroids(query)?;
            let mut scored_vectors = Vec::new();
            for c in clusters {
                let index = c.0;
                assert!(index < self.lists.len());

                for id in &self.lists[index] {
                    let v = self.vectors.get(id).expect("must has id");
                    let score = M::score(query, v)?;
                    scored_vectors.push(ScoredId::new(id.clone(), score));
                }
            }
            scored_vectors
        };

        scored_vectors.sort_by(|a, b| {
            b.score.partial_cmp(&a.score).unwrap()
        });
    
        scored_vectors.truncate(top_k);
        Ok(scored_vectors)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.vectors.clear();
        self.centroids.clear();
        self.lists = vec![vec![]; self.n_clusters];
        Ok(())
    }

    fn len(&self) -> usize {
        self.vectors.len()
    }
}
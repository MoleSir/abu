mod flat;
pub use flat::*;
mod lsh;
pub use lsh::*;
mod ivf;
pub use ivf::*;

use super::{Float, VectorId};

#[derive(Clone, Debug)]
pub struct ScoredId<F: Float> {
    pub id: VectorId,
    pub score: F,
}

#[allow(async_fn_in_trait)]
pub trait VectorIndex: Send + Sync {
    type F: Float;
    type Error: std::error::Error + 'static + Send + Sync;

    async fn add(&mut self, id: VectorId, vector: Vec<Self::F>) -> Result<(), Self::Error>;
    async fn add_batch<IS, VS>(&mut self, ids: IS, vectors: VS) -> Result<(), Self::Error>
    where 
        IS: IntoIterator<Item = VectorId>,
        VS: IntoIterator<Item = Vec<Self::F>>,
    {
        for (id, vector) in ids.into_iter().zip(vectors.into_iter()) {
            self.add(id, vector).await?;
        }
        Ok(())
    }

    async fn delete(&mut self, id: VectorId) -> Result<(), Self::Error>;
    async fn delete_batch<IS>(&mut self, ids: IS) -> Result<(), Self::Error> 
    where 
        IS: IntoIterator<Item = VectorId>,
    {
        for id in ids {
            self.delete(id).await?;
        }
        Ok(())
    }

    async fn search(&self, query: &[Self::F], top_k: usize) -> Result<Vec<ScoredId<Self::F>>, Self::Error>;

    async fn clear(&mut self) -> Result<(), Self::Error>;

    fn len(&self) -> usize;
}

impl<F: Float> ScoredId<F> {
    pub fn new(id: VectorId, score: F) -> Self {
        Self { id, score }
    }
}


// #[derive(Clone, Debug)]
// pub struct ScoredId {
//     pub id: VectorId,
//     pub score: f32,
// }

// #[allow(async_fn_in_trait)]
// pub trait VectorIndex: Send + Sync {
//     type Error: std::error::Error + 'static + Send + Sync;

//     async fn add(&mut self, id: VectorId, vector: Vec<f32>) -> Result<(), Self::Error>;
//     async fn add_batch<IS, VS>(&mut self, ids: IS, vectors: VS) -> Result<(), Self::Error>
//     where 
//         IS: IntoIterator<Item = VectorId>,
//         VS: IntoIterator<Item = Vec<f32>>,
//     {
//         for (id, vector) in ids.into_iter().zip(vectors.into_iter()) {
//             self.add(id, vector).await?;
//         }
//         Ok(())
//     }

//     async fn delete(&mut self, id: VectorId) -> Result<(), Self::Error>;
//     async fn delete_batch<IS>(&mut self, ids: IS) -> Result<(), Self::Error> 
//     where 
//         IS: IntoIterator<Item = VectorId>,
//     {
//         for id in ids {
//             self.delete(id).await?;
//         }
//         Ok(())
//     }

//     async fn search(&self, query: &[f32], top_k: usize) -> Result<Vec<ScoredId>, Self::Error>;

//     async fn clear(&mut self) -> Result<(), Self::Error>;

//     fn len(&self) -> usize;
// }

// impl ScoredId {
//     pub fn new(id: VectorId, score: f32) -> Self {
//         Self { id, score }
//     }
// }


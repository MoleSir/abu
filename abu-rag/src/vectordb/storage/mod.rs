#[cfg(feature = "sqlite")]
pub mod sqlite;

mod memory;
pub use memory::*;

use std::borrow::Cow;
use super::VectorId;

#[allow(async_fn_in_trait)]
pub trait VectorStorage: Send + Sync {
    type Payload: Clone + 'static + Send + Sync;
    type Error: std::error::Error + 'static + Send + Sync;

    async fn add(&mut self, id: VectorId, payload: Self::Payload) -> Result<(), Self::Error>;
    async fn add_batch<IS, PS>(&mut self, ids: IS, payloads: PS) -> Result<(), Self::Error>
    where 
        IS: IntoIterator<Item = VectorId>,
        PS: IntoIterator<Item = Self::Payload>,
    {
        for (id, payload) in ids.into_iter().zip(payloads.into_iter()) {
            self.add(id, payload).await?;
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
    
    async fn get(&self, id: VectorId) -> Result<Option<Cow<Self::Payload>>, Self::Error>;

    async fn clear(&mut self) -> Result<(), Self::Error>;
}

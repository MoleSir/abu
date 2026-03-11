use std::{borrow::Cow, collections::HashMap, convert::Infallible};
use crate::vectordb::VectorId;
use super::VectorStorage;

pub struct InMemoryStorage<P> {
    payloads: HashMap<VectorId, P>
}

impl<P> InMemoryStorage<P> {
    pub fn new() -> Self {
        Self { payloads: HashMap::new() }
    }
}

impl<P: Clone + 'static + Send + Sync> VectorStorage for InMemoryStorage<P> {
    type Payload = P;
    type Error = Infallible;

    async fn add(&mut self, id: VectorId, payload: P) -> Result<(), Self::Error> {
        self.payloads.insert(id, payload);
        Ok(())
    }

    async fn delete(&mut self, id: VectorId) -> Result<(), Self::Error> {
        self.payloads.remove(&id);
        Ok(())
    }

    async fn get(&self, id: VectorId) -> Result<Option<Cow<P>>, Self::Error> {
        let value = self.payloads.get(&id);
        Ok(value.map(|v| Cow::Borrowed(v)))
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.payloads.clear();
        Ok(())
    }
} 
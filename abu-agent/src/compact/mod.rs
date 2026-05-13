mod slidingwindow;
pub use slidingwindow::*;
mod summary;
pub use summary::*;

use std::{convert::Infallible, fmt::Display};

use crate::AgentContext;

#[async_trait::async_trait]
pub trait ContextCompact : Send + Sync {
    type Error: Display + 'static + Send + Sync;

    async fn compact(&mut self, context: &mut AgentContext) -> Result<(), Self::Error>;
}

pub struct NoContextCompact;

#[async_trait::async_trait]
impl ContextCompact for NoContextCompact {
    type Error = Infallible;

    async fn compact(&mut self, _context: &mut AgentContext) -> Result<(), Self::Error> {
        Ok(())
    }
}
use std::convert::Infallible; 
use crate::AgentContext;
use super::ContextCompact;

pub struct SliceWindowCompact {
    window_size: usize,
}

impl SliceWindowCompact {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
        }
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }
}

#[async_trait::async_trait]
impl ContextCompact for SliceWindowCompact {
    type Error = Infallible;

    async fn compact(&mut self, context: &mut AgentContext) -> Result<(), Self::Error> {
        if context.conversations.len() > self.window_size {
            return Ok(())
        }

        let mut conversations = vec![];
        std::mem::swap(&mut conversations, &mut context.conversations); 

        let skip_size = conversations.len() - self.window_size;
        context.conversations = conversations.into_iter().skip(skip_size).collect();

        Ok(())
    }
}
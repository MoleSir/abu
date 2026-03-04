use std::collections::VecDeque; 
use abu_api::chat::ChatMessage;
use super::Memory;

pub struct SliceWindowMemory {
    history: VecDeque<ChatMessage>, 
    window_size: usize,
}

impl SliceWindowMemory {
    pub fn new(window_size: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(window_size),
            window_size,
        }
    }

    pub fn window_size(&self) -> usize {
        self.window_size
    }
}

#[async_trait::async_trait]
impl Memory for SliceWindowMemory {
    async fn fork(&self) -> anyhow::Result<Box<dyn Memory>> {
        Ok(Box::new(Self::new(self.window_size)))
    }

    async fn add_message(&mut self, message: ChatMessage) -> anyhow::Result<()> {
        if self.history.len() >= self.window_size {
            self.history.pop_front();
        }
        self.history.push_back(message);
        Ok(())
    }

    async fn compact_messages(&mut self, _query: &str) -> anyhow::Result<Vec<ChatMessage>> {
        Ok(self.history.iter().cloned().collect::<Vec<_>>())
    }

    async fn clear(&mut self) -> anyhow::Result<()> {
        self.history.clear();
        Ok(())
    }
}
use std::collections::VecDeque; 
use abu_api::chat::ChatMessage;
use super::{MemoryStrategy, MemoryResult};

pub struct SliceWindow {
    history: VecDeque<ChatMessage>, 
    window_size: usize,
}

impl SliceWindow {
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
impl MemoryStrategy for SliceWindow {
    async fn add_message(&mut self, message: ChatMessage) -> MemoryResult<()> {
        if self.history.len() >= self.window_size {
            self.history.pop_front();
        }
        self.history.push_back(message);
        Ok(())
    }

    async fn compact_messages(&mut self, _query: &str) -> MemoryResult<Vec<ChatMessage>> {
        Ok(self.history.iter().cloned().collect::<Vec<_>>())
    }

    async fn clear(&mut self) -> MemoryResult<()> {
        self.history.clear();
        Ok(())
    }
}
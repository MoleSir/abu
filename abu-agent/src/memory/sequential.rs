use abu_api::chat::ChatMessage;
use super::{MemoryStrategy, MemoryResult};

pub struct Sequential {
    history: Vec<ChatMessage>,
}

impl Sequential {
    pub fn new() -> Self {
        Self { history: vec![] }
    }
}

#[async_trait::async_trait]
impl MemoryStrategy for Sequential {
    async fn add_message(&mut self, message: ChatMessage) -> MemoryResult<()> {
        self.history.push(message);
        Ok(())
    }

    async fn compact_messages(&mut self, _query: &str) -> MemoryResult<Vec<&ChatMessage>> {
        Ok(self.history.iter().collect())
    }

    async fn clear(&mut self) -> MemoryResult<()> {
        self.history.clear();
        Ok(())
    }
}

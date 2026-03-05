use abu_api::chat::ChatMessage;
use super::Memory;

pub struct SequentialMemory {
    history: Vec<ChatMessage>,
}

impl SequentialMemory {
    pub fn new() -> Self {
        Self { history: vec![] }
    }
}

#[async_trait::async_trait]
impl Memory for SequentialMemory {
    async fn fork(&self) -> anyhow::Result<Box<dyn Memory>> {
        Ok(Box::new(Self::new()))
    }

    async fn add_message(&mut self, message: ChatMessage) -> anyhow::Result<()> {
        self.history.push(message);
        Ok(())
    }

    async fn compact_messages(&mut self, _query: &str) -> anyhow::Result<Vec<ChatMessage>> {
        Ok(self.history.clone())
    }

    async fn clear(&mut self) -> anyhow::Result<()> {
        self.history.clear();
        Ok(())
    }
}

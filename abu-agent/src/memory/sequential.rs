use std::convert::Infallible;
use abu_base::chat::ChatMessage;
use super::Memory;

#[derive(Default)]
pub struct SequentialMemory {
    history: Vec<ChatMessage>,
}

impl SequentialMemory {
    pub fn new() -> Self {
        Self { history: vec![] }
    }
}

impl Memory for SequentialMemory {
    type Error = Infallible;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error> {
        let user_message = ChatMessage::user(user_input);
        let ai_message = ChatMessage::assistant(ai_response, []);
        self.history.push(user_message);
        self.history.push(ai_message);
        Ok(())
    }
    
    async fn search(&mut self, _query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        Ok(self.history.clone())
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.history.clear();
        Ok(())
    }
}

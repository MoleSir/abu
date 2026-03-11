use std::{collections::VecDeque, convert::Infallible}; 
use abu_base::chat::ChatMessage;
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

impl Memory for SliceWindowMemory {
    type Error = Infallible;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error> {
        while self.history.len() / 2 > self.window_size {
            self.history.pop_front(); // Pop User
            self.history.pop_front(); // Pop Assistant
        }
        self.history.push_back(ChatMessage::user(user_input));
        self.history.push_back(ChatMessage::assistant(ai_response, []));
        Ok(())
    }

    async fn search(&self, _query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        Ok(self.history.iter().cloned().collect())
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.history.clear();
        Ok(())
    }
}
use abu_base::chat::ChatMessage;
use abu_provider::EmbedProvide;
use super::{retrieval::RetrievalMemoryError, Memory, RetrievalMemory, SliceWindowMemory};

pub struct HierarchicalMemory<P> {
    working_memory: SliceWindowMemory,
    long_term_memory: RetrievalMemory<P>,
    promotion_keywords: Vec<&'static str>,
}

impl<P: EmbedProvide> HierarchicalMemory<P> {
    pub fn new(window_size: usize, provider: P, model: impl Into<String>, top_k: usize) -> Self {
        let working_memory = SliceWindowMemory::new(window_size);
        let long_term_memory = RetrievalMemory::new(provider, model, top_k);
        Self {
            working_memory, 
            long_term_memory,
            promotion_keywords: vec![
                "remember", "rule", "preference", "always", "never", "allergic"
            ],
        }
    }
}

#[async_trait::async_trait]
impl<P: EmbedProvide> Memory for HierarchicalMemory<P> {
    type Error = RetrievalMemoryError;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error> {
        self.working_memory.add(user_input, ai_response).await.unwrap();
        
        let user_input_lower = user_input.to_lowercase();
        let has_keyword = self.promotion_keywords.iter()
            .any(|&promotion_keyword| user_input_lower.contains(promotion_keyword));
        if has_keyword {
            self.long_term_memory.add(user_input, ai_response).await?;
        }

        Ok(())
    }
    
    async fn search(&self, query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        let working_messages = self.working_memory.search(query).await.unwrap();
        let long_term_messages = self.long_term_memory.search(query).await?;
        
        let mut messages = vec![];
        messages.push(ChatMessage::user("Retrieved Long-Term Memories:"));
        messages.extend(long_term_messages);
        messages.push(ChatMessage::user("Recent Conversation (Working Memory):"));
        messages.extend(working_messages);

        Ok(messages)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.working_memory.clear().await.unwrap();
        self.long_term_memory.clear().await?;
        Ok(())
    }
}

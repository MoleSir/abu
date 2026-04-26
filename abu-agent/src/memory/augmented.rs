use abu_base::chat::{ChatMessage, ChatRequestBuilder};
use abu_provider::ChatProvide;
use tracing::debug;
use crate::{AgentCtxError, AgentError};
use super::{Memory, SliceWindowMemory};

pub struct AugmentedMemory<P> {
    llm: P,
    model: String,
    recent_memory: SliceWindowMemory,
    memory_tokens: Vec<String>,
}

impl<P: ChatProvide> AugmentedMemory<P> {
    pub fn new(llm: P, model: impl Into<String>, window_size: usize) -> Self {
        Self { 
            llm,
            model: model.into(),
            recent_memory: SliceWindowMemory::new(window_size),
            memory_tokens: vec![] 
        }
    }
}

#[async_trait::async_trait]
impl<P: ChatProvide> Memory for AugmentedMemory<P> {
    type Error = AgentCtxError;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error> {
        self.recent_memory.add(user_input, ai_response).await.expect("recent_memory error");
        

        let fact_extraction_prompt = format!(
            "Analyze the following conversation turn. Does it contain a core fact, preference, or decision that should be remembered long-term? \
             Examples include user preferences ('I hate flying'), key decisions ('The budget is $1000'), or important facts ('My user ID is 12345').\n\n\
             Conversation Turn:\nUser: {user_input}\nAI: {ai_response}\n\n\
             If it contains such a fact, state the fact concisely in one sentence. Otherwise, respond with 'No important fact.'"
        );

        let request = ChatRequestBuilder::default()
            .model(&self.model)
            .messages(vec![
                ChatMessage::system("You are a fact-extraction expert."),
                ChatMessage::user(fact_extraction_prompt),
            ])
            .build()?;
        
        let response = self.llm
            .chat(&request).await
            .map_err(|e| AgentError::ChatProvider(Box::new(e)))?
            .message;

        // lowcase?
        if !response.content.contains("No important fact") {
            let extracted_fact = response.content;
            debug!("--- [Memory Augmentation: New memory token created: '{}'] ---", extracted_fact);
            self.memory_tokens.push(extracted_fact);
        }

        Ok(())
    }
    
    async fn search(&self, query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        let recent_context = self.recent_memory.search(query).await.expect("recent_memory error");
        let mut context = vec![
            ChatMessage::user(format!("### Key Memory Tokens (Long-Term Facts):\n{}\n\n", self.memory_tokens.join("\n"))),
            ChatMessage::user("### Recent Conversation:\n")
        ];
        context.extend(recent_context);
        Ok(context)
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.recent_memory.clear().await.expect("rec mem err");
        self.memory_tokens.clear();
        Ok(())
    }
}

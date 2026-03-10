use abu_base::chat::{ChatMessage, ChatRequestBuilder};
use abu_provider::ChatProvide;
use tracing::debug;
use crate::{AgentCtxError, AgentError, AgentResult};

use super::Memory;

pub struct SummarizationMemory<P> {
    llm: P,
    model: String,
    messages: Vec<ChatMessage>,
    summary_threshold: usize,
}

impl<P: ChatProvide> SummarizationMemory<P> {
    pub fn new(llm: P, model: impl Into<String>, summary_threshold: usize) -> Self {
        Self { 
            llm,
            model: model.into(),
            messages: vec![], 
            summary_threshold,
        }
    }

    pub fn user_message_count(&self) -> usize {
        self.messages.iter()
            .filter(|m| m.is_user())
            .count()
    }

    /// call llm to summary `messages` and reset `messages`
    async fn consolidate_memory(&mut self) -> AgentResult<()> {
        debug!("--- [Memory Consolidation Triggered] ---");

        // collection all messages
        let buffer_text = self.messages.iter()
            .map(|m| Self::format_message(m))
            .collect::<Vec<_>>()
            .join("\n");

        // send to llm
        let summarization_prompt = format!(
           "Summarize this conversation for continuity. Include:  \
            1) What was accomplished, 2) Current state, 3) Key decisions made. \
            Be concise but preserve critical details.\n\n{}",
            buffer_text
        );
        let messages = vec![
            ChatMessage::system("You are an expert summarization engine."),
            ChatMessage::user(summarization_prompt),
        ];
        let request = ChatRequestBuilder::default()
            .model(&self.model)
            .messages(messages)
            .temperature(0.7)
            .build()?;
        let response = self.llm
            .chat(&request).await
            .map_err(|e| AgentError::ChatProvider(Box::new(e)))?
            .message;
        
        // update messages
        self.messages.clear();
        self.messages.push(ChatMessage::user(format!("[Conversation compressed]: {}", response.content)));
        self.messages.push(ChatMessage::assistant("Understood. I have the context from the summary. Continuing.", []));

        Ok(())
    }

    fn format_message(msg: &ChatMessage) -> String {
        format!("{}: {}", msg.role(), msg.content())
    }
}

impl<P: ChatProvide> Memory for SummarizationMemory<P> {
    type Error = AgentCtxError;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error> {
        self.messages.push(ChatMessage::user(user_input));
        self.messages.push(ChatMessage::assistant(ai_response, []));
        if self.user_message_count() >= self.summary_threshold {
            self.consolidate_memory().await?;
        }
        Ok(())
    }

    async fn search(&mut self, _query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        Ok(self.messages.clone())
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.messages.clear();
        Ok(())
    }
}

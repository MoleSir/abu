use abu_base::chat::ChatMessage;
use abu_provider::ChatProvide;
use tracing::debug;
use crate::{model::ChatModel, AgentCtxError, AgentResult};

use super::Memory;

pub struct SummarizationMemory<P> {
    llm: ChatModel<P>,
    messages: Vec<ChatMessage>,
    summary_threshold: usize,
}

impl<P: ChatProvide> SummarizationMemory<P> {
    pub fn new(llm: ChatModel<P>, summary_threshold: usize) -> Self {
        Self { 
            llm,
            messages: vec![], 
            summary_threshold,
        }
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
        let response = self.llm.chat(messages).await?.message;
        
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
        if self.messages.len() / 2 >= self.summary_threshold {
            self.consolidate_memory().await?;
        }
        Ok(())
    }

    async fn search(&self, _query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        Ok(self.messages.clone())
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        self.messages.clear();
        Ok(())
    }
}

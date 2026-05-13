use abu_base::chat::ChatMessage;
use abu_provider::ChatProvide;
use crate::{model::ChatModel, AgentContext, AgentError};
use super::ContextCompact;

pub struct SummarizationCompact<P> {
    llm: ChatModel<P>,
    summary_threshold: usize,
}

impl<P: ChatProvide> SummarizationCompact<P> {
    pub fn new(llm: ChatModel<P>, summary_threshold: usize) -> Self {
        Self { 
            llm,
            summary_threshold,
        }
    }

    fn format_message(msg: &ChatMessage) -> String {
        format!("{}: {}", msg.role(), msg.content())
    }
}

#[async_trait::async_trait]
impl<P: ChatProvide> ContextCompact for SummarizationCompact<P> {
    type Error = AgentError;

    async fn compact(&mut self, context: &mut AgentContext) -> Result<(), Self::Error> {
        if context.conversations.len() + context.memory.len() + 1 > self.summary_threshold {
            return Ok(());
        }
        
        // collection all messages
        let buffer_text = context.conversations.iter()
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

        context.conversations = vec![];
        context.conversations.push(ChatMessage::user(format!("[Conversation compressed]: {}", response.content)));
        context.conversations.push(ChatMessage::assistant("Understood. I have the context from the summary. Continuing.", []));

        Ok(())
    }
}

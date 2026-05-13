use abu_base::chat::ChatMessage;

pub struct AgentContext {
    pub system_prompt: String,
    pub memory: Vec<ChatMessage>,
    pub conversations: Vec<ChatMessage>,
} 

impl AgentContext {
    pub fn assemble_messages(&self) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage::system(self.system_prompt.clone())];
        messages.extend(self.memory.clone());
        messages.extend(self.conversations.clone());
        messages
    }
}

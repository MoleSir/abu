use abu_base::chat::ChatMessage;

pub struct ContextBuilder {
    pub system_prompt: String,
}

impl ContextBuilder {
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self { system_prompt: system_prompt.into() }
    }

    pub fn system_prompt(&self) -> &str {
        self.system_prompt.as_str()
    }

    pub fn assemble(&self, memories: Vec<ChatMessage>, middleware_context: String, query: &str) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage::system(format!("{}\n\n{}", self.system_prompt, middleware_context))];
        messages.extend(memories);
        messages.push(ChatMessage::user(query));
        messages
    }
}

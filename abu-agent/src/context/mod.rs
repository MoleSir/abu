use std::sync::Arc;
use abu_base::chat::ChatMessage;
use abu_skill::SkillLoader;

pub struct ContextBuilder {
    pub system_prompt: String,   
    pub skill_loader: Option<Arc<SkillLoader>>,
}

impl ContextBuilder {
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self { 
            system_prompt: system_prompt.into(),
            skill_loader: None,
        }
    }
    
    pub fn with_skill(&mut self, skill: Arc<SkillLoader>) {
        self.skill_loader = Some(skill);
    }

    pub fn build(&self, query: &str, memories: Vec<ChatMessage>) -> Vec<ChatMessage> {
        let system_prompt = self.build_system_prompt();
        let mut messages = vec![ChatMessage::system(system_prompt)];
        messages.extend(memories);
        messages.push(ChatMessage::user(query));
        messages
    }

    fn build_system_prompt(&self) -> String {
        /*
        TODO:
            用 占位符 替代硬编码处理时间、路径等环境上下文。
            用 push_str 优化字符串拼接。
            加一道 Memory 长度/Token 保护机制。
        */
        let mut contents = vec![self.system_prompt.clone()];
        if let Some(skill_loader) = self.skill_loader.as_ref() {
            contents.push(skill_loader.get_descriptions());
        }
        contents.join("\n\n")
    }
}


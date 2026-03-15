use abu_base::chat::ChatMessage;

pub struct ContextBuilder {
    pub system_prompt: String,   
}

impl ContextBuilder {
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self { system_prompt: system_prompt.into() }
    }
    
    pub fn build(&self, query: &str, memorys: Vec<ChatMessage>) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage::system(self.system_prompt.clone())];
        messages.extend(memorys);
        messages.push(ChatMessage::user(query));
        messages
    }
}

// pub struct ContextBuilder {
//     pub system_prompt: String,   
//     pub skill_loader: Option<SkillLoader>,
// }

// impl ContextBuilder {
//     pub fn new<S, P>(system_prompt: S, skill_path: Option<P>) -> AgentResult<Self> 
//     where 
//         S: Into<String>,
//         P: Into<PathBuf>
//     {
//         let mut system_prompt: String = system_prompt.into();
//         let skill_loader = match skill_path {
//             Some(path) => {
//                 let skill_loader = SkillLoader::load(path).context("load skill")?;
//                 system_prompt = format!("{}\n\n{}", system_prompt, skill_loader.get_descriptions());
//                 Some(skill_loader)
//             }
//             None => None,
//         };
    
//         Ok(Self { system_prompt, skill_loader })
//     }

//     pub fn build(&self, query: &str, memorys: Vec<ChatMessage>) -> Vec<ChatMessage> {
//         let mut messages = vec![ChatMessage::system(self.system_prompt.clone())];
//         messages.extend(memorys);
//         messages.push(ChatMessage::user(query));
//         messages
//     }
// }
use abu_skill::SkillLoader;
use std::{convert::Infallible, sync::Arc};
use crate::middleware::{MiddlewareFlow, SystemPromptMiddleware};

pub struct SkillTool {
    pub skill_loader: Arc<SkillLoader>,
}

impl SkillTool {
    pub fn new(skill_loader: Arc<SkillLoader>) -> Self {
        Self { skill_loader }
    }
}

#[abu_macros::tool(
    struct_name = SkillTool,
    description = "Load specialized skill by name.",
    name = "load_skill",
)]
pub fn load(&self, name: &str) -> String {
    self.skill_loader.get_content(name)
        .map(|c| c.to_string())
        .unwrap_or(format!("no skill {name}"))
}

pub struct SkillMiddleware {
    pub skill_loader: Arc<SkillLoader>,
}

impl SkillMiddleware {
    pub fn new(skill_loader: Arc<SkillLoader>) -> Self {
        Self { skill_loader }
    }
}

#[async_trait::async_trait]
impl SystemPromptMiddleware for SkillMiddleware {
    type Error = Infallible;
    async fn intercept(&mut self, prompt: &mut String) -> Result<MiddlewareFlow, Self::Error> {
        let context = self.skill_loader.get_descriptions();
        prompt.push_str(&context);
        Ok(MiddlewareFlow::Continue)
    }
}
use std::{convert::Infallible, sync::Arc};
use abu_skill::SkillLoader;
use super::{MiddlewareFlow, SystemPromptMiddleware};

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
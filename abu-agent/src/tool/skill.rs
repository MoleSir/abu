use std::sync::Arc;
use abu_skill::SkillLoader;

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
    description = "Load specialized knowledge by name.",
    name = "load_skill",
)]
pub fn load(&self, name: &str) -> String {
    self.skill_loader.get_content(name)
        .map(|c| c.to_string())
        .unwrap_or(format!("no skill {name}"))
}

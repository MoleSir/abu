use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Skill {
    pub frontmatter: SkillFrontmatter,
    pub body: String,

}

/// Frontmatter metadata for a skill.
/// https://agentskills.io/specification
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillFrontmatter {
    /// A required short identifier (1-64 chars) containing only lowercase letters, numbers, and hyphens.
    pub name: String,
    /// A required concise description of what the skill does and when an agent should use it.
    pub description: String,
    /// License name or reference to a bundled license file.
    pub license: Option<String>,
    /// Max 500 characters. Indicates environment requirements (intended product, system packages, network access, etc.).
    pub compatibility: Option<String>,
    /// Arbitrary key-value mapping for additional metadata.
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    /// Space-delimited list of pre-approved tools the skill may use. (Experimental)
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Vec<String>,
}

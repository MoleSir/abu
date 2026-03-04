use std::{collections::HashMap, path::{Path, PathBuf}, sync::OnceLock};
use crate::{Skill, SkillError, SkillFrontmatter, SkillResult};
use regex::Regex;
use walkdir::{WalkDir, DirEntry};
use tracing::debug;

pub struct SkillLoader {
    pub dir: PathBuf,
    pub skills: HashMap<String, Skill>,
}

impl SkillLoader {
    pub fn load(skill_dir: impl Into<PathBuf>) -> SkillResult<Self> {
        let skill_dir: PathBuf = skill_dir.into();
        debug!("load skills from {}", skill_dir.display());
        let skills = Self::load_skills(&skill_dir)?;
        Ok(Self { dir: skill_dir, skills })
    }

    pub fn get_descriptions(&self) -> String {
        if self.skills.is_empty() {
            "(no skills available)".to_string()
        } else {
            let skill_descriptions = self.skills.iter()
                .map(|(name, skill)| {
                    format!("  - {}: {}", name, skill.frontmatter.description)
                })
                .collect::<Vec<_>>()
                .join("\n");
            
            format!("Use load_skill to access full content of one skill.\nHere are all available skills for you:\n{}", skill_descriptions)
        }
    }

    pub fn get_content(&self, name: &str) -> Option<&str> {
        self.skills
            .get(name)
            .map(|skill| skill.body.as_str())
    }

    fn load_skills(skill_dir: &Path) -> SkillResult<HashMap<String, Skill>> {
        WalkDir::new(skill_dir).min_depth(1).max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(Self::check_skill_path)
            .map(|(name, path)| {
                debug!("load skill {} from {}", name, path.display());
                Self::load_skill(&path).map(|skill| (name, skill))
            })
            .collect()
    }

    fn load_skill(path: &Path) -> SkillResult<Skill> {
        let content = std::fs::read_to_string(path)?;
        Self::parse_skill(&content)
    }

    fn parse_skill(content: &str) -> SkillResult<Skill> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| { Regex::new(r"(?s)^---\r?\n(.*?)\r?\n---\r?\n(.*)").expect("Invalid Regex") });

        if let Some(caps) = re.captures(&content) {
            let yaml_str = caps.get(1).map_or("", |m| m.as_str());
            let body_str = caps.get(2).map_or("", |m| m.as_str());
            let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml_str)?;
            Ok(Skill {
                frontmatter,
                body: body_str.trim().to_string(), 
            })
        } else {
            return Err(SkillError::InvalidFrontmatter {
                message: "missing opening frontmatter delimiter (`---`)".to_string(),
            });
        }
    }

    fn check_skill_path(entry: DirEntry) -> Option<(String, PathBuf)> {
        // type 1: .md file
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|ext| ext == "md") {
            Some((entry.file_name().to_str().unwrap().to_string(), entry.path().to_owned()))
        } else if entry.file_type().is_dir() {
            let md_path = entry.path().join("SKILL.md");
            // type 2: SKILL.md in sub dir
            if md_path.exists() {
                Some((entry.file_name().to_str().unwrap().to_string(), md_path))
            } else {
                None
            }
        } else {
            None
        }
    } 
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader() {
        let loader = SkillLoader::load("./skills").expect("load");
        assert_eq!(loader.skills.len(), 2);
    }

    #[test]
    fn test_parses_valid_skill() {
        let content = r#"---
name: repo_search
description: Search the codebase quickly
---
Use ripgrep first.
"#;
        let skill = SkillLoader::parse_skill(content).unwrap();
        assert_eq!(skill.frontmatter.name, "repo_search");
        assert_eq!(skill.frontmatter.description, "Search the codebase quickly");
        assert!(skill.body.contains("Use ripgrep first."));
    }

    #[test]
    fn test_parses_skill_with_full_spec() {
        let content = r#"---
name: full_spec_agent
description: An agent with everything
compatibility: "Requires Python 3.10+"
allowed-tools:
  - tool1
references:
  - ref1
metadata:
  custom_key: custom_value
  version: "1.2.3"
  license: MIT
---
Body content.
"#;
        let skill = SkillLoader::parse_skill(content).unwrap();
        assert_eq!(skill.frontmatter.name, "full_spec_agent");
        assert_eq!(skill.frontmatter.compatibility, Some("Requires Python 3.10+".to_string()));
        assert_eq!(skill.frontmatter.allowed_tools, vec!["tool1"]);
        assert_eq!(
            skill.frontmatter.metadata.get("custom_key").and_then(|v| v.as_str()),
            Some("custom_value")
        );
    }
}
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use abu_agent::middleware::{MiddlewareFlow, SystemPromptMiddleware};
use regex::Regex;
use tokio::sync::RwLock;
use walkdir::WalkDir;

const MEMORY_GUIDANCE: &str = r#"## Memory system

You have a persistent, file-based memory system. Save important information that will be useful in future sessions.

### When to save memories
- User states a preference ("I like tabs", "always use pytest") -> type: user
- User corrects you ("don't do X", "that was wrong because...") -> type: feedback
- You learn a non-obvious project fact (compliance rules, business constraints) -> type: project
- You learn where an external resource lives (ticket board, dashboard, docs URL) -> type: reference

### When NOT to save
- Code patterns, conventions, file paths (derivable from code)
- Git history or recent changes (use git log)
- Debugging solutions (the fix is in the code)
- Temporary task state (current branch, open PRs)
- Secrets or credentials

Use the save_memory tool to persist memories."#;

// ============================================================================
// MemoryMiddleware
// ============================================================================

pub struct MemoryMiddleware {
    pub memory_manager: Arc<RwLock<MemoryManager>>,
}

impl MemoryMiddleware {
    pub fn new(memory_manager: Arc<RwLock<MemoryManager>>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait::async_trait]
impl SystemPromptMiddleware for MemoryMiddleware {
    type Error = anyhow::Error;

    async fn intercept(&mut self, prompt: &mut String) -> Result<MiddlewareFlow, Self::Error> {
        let memories_text = self.memory_manager.read().await.load_memory_prompt();
        prompt.push_str(&memories_text);
        prompt.push('\n');
        prompt.push_str(MEMORY_GUIDANCE);
        Ok(MiddlewareFlow::Continue)
    }
}

// ============================================================================
// MemoryTool
// ============================================================================

pub struct MemoryTool {
    pub memory_manager: Arc<RwLock<MemoryManager>>,
}

impl MemoryTool {
    pub fn new(memory_manager: Arc<RwLock<MemoryManager>>) -> Self {
        Self { memory_manager }
    }
}

#[abu_tool::tool(
    struct_name = MemoryTool,
    name = "save_memory",
    description = "Save a persistent memory that survives across sessions."
)]
pub async fn save_memory(
    &self,
    #[arg(description = "Short identifier (e.g. prefer_tabs, db_schema)")]
    name: &str,
    #[arg(description = "One-line summary")]
    description: &str,
    #[arg(description = "user=preferences, feedback=corrections, project=non-obvious conventions/decisions, reference=external resource pointers")]
    kind: MemoryKind,
    #[arg(description = "Full memory content")]
    content: &str,
) -> anyhow::Result<String> {
    self.memory_manager
        .write()
        .await
        .save_memory(name, description, kind, content)
}

// ============================================================================
// Memory kinds
// ============================================================================

#[abu_tool::tool_argument]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum MemoryKind {
    User,
    Feedback,
    Project,
    Reference,
}

impl MemoryKind {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "user" => Self::User,
            "feedback" => Self::Feedback,
            "project" => Self::Project,
            "reference" => Self::Reference,
            _ => Self::Project,
        }
    }
}

// ============================================================================
// Memory model
// ============================================================================

pub struct Memory {
    pub name: String,
    pub description: String,
    pub kind: MemoryKind,
    pub content: String,
    #[allow(dead_code)]
    pub file: String,
}

// ============================================================================
// MemoryManager
// ============================================================================

pub struct MemoryManager {
    pub memory_dir: PathBuf,
    pub memories: HashMap<String, Memory>,
}

impl MemoryManager {
    pub fn new<P: Into<PathBuf>>(memory_dir: P) -> anyhow::Result<Self> {
        let memory_dir = memory_dir.into();
        let mut manager = Self {
            memory_dir,
            memories: HashMap::new(),
        };
        manager.load_all()?;
        Ok(manager)
    }

    pub fn load_all(&mut self) -> anyhow::Result<()> {
        self.memories.clear();
        if !self.memory_dir.exists() {
            return Ok(());
        }

        for entry in WalkDir::new(&self.memory_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map_or(false, |ext| ext == "md")
            })
        {
            if entry.file_name() == "MEMORY.md" {
                continue;
            }
            let content = std::fs::read_to_string(entry.path())?;
            if let Some(mem) = self.parse_frontmatter(&content) {
                self.memories.insert(mem.name.clone(), mem);
            }
        }

        Ok(())
    }

    pub fn save_memory(
        &mut self,
        name: &str,
        description: &str,
        kind: MemoryKind,
        content: &str,
    ) -> anyhow::Result<String> {
        let safe_name: String = name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        std::fs::create_dir_all(&self.memory_dir)?;

        let file_name = format!("{}.md", safe_name);
        let file_path = self.memory_dir.join(&file_name);

        let full_content = format!(
            "---\nname: {}\ndescription: {}\ntype: {}\n---\n{}\n",
            name,
            description,
            kind.to_str(),
            content
        );

        std::fs::write(&file_path, full_content)?;

        self.memories.insert(
            name.to_string(),
            Memory {
                name: name.to_string(),
                description: description.to_string(),
                kind,
                content: content.to_string(),
                file: file_name,
            },
        );

        self.rebuild_index()?;

        Ok(format!(
            "Saved memory '{}' [{}]",
            name,
            kind.to_str()
        ))
    }

    pub fn load_memory_prompt(&self) -> String {
        if self.memories.is_empty() {
            return String::new();
        }

        let mut sections = vec!["# Memories (persistent across sessions)\n".to_string()];

        for kind in [
            MemoryKind::Project,
            MemoryKind::Feedback,
            MemoryKind::User,
            MemoryKind::Reference,
        ] {
            let typed: Vec<&Memory> = self
                .memories
                .values()
                .filter(|m| m.kind == kind)
                .collect();

            if typed.is_empty() {
                continue;
            }

            sections.push(format!("## [{}]", kind.to_str()));
            for mem in typed {
                sections.push(format!("### {}: {}", mem.name, mem.description));
                if !mem.content.is_empty() {
                    sections.push(mem.content.clone());
                }
                sections.push(String::new());
            }
        }

        sections.join("\n")
    }

    fn parse_frontmatter(&self, text: &str) -> Option<Memory> {
        let re = Regex::new(r"(?s)^---\s*\n(.*?)\n---\s*\n(.*)").ok()?;
        let caps = re.captures(text)?;
        let header = caps.get(1)?.as_str();
        let body = caps.get(2)?.as_str().trim();

        let mut name = String::new();
        let mut description = String::new();
        let mut kind = MemoryKind::Project;

        for line in header.lines() {
            if let Some((key, value)) = line.split_once(':') {
                match key.trim() {
                    "name" => name = value.trim().to_string(),
                    "description" => description = value.trim().to_string(),
                    "type" => kind = MemoryKind::from_str(value.trim()),
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            return None;
        }

        Some(Memory {
            name,
            description,
            kind,
            content: body.to_string(),
            file: String::new(),
        })
    }

    fn rebuild_index(&self) -> anyhow::Result<()> {
        let mut lines = vec![String::from("# Memory Index"), String::new()];

        let mut sorted_names: Vec<_> = self.memories.keys().collect();
        sorted_names.sort();

        for name in sorted_names {
            let mem = &self.memories[name];
            lines.push(format!(
                "- {}: {} [{}]",
                name,
                mem.description,
                mem.kind.to_str()
            ));
            if lines.len() > 200 {
                lines.push("... (truncated)".to_string());
                break;
            }
        }

        let index_path = self.memory_dir.join("MEMORY.md");
        std::fs::write(index_path, lines.join("\n") + "\n")?;

        Ok(())
    }

    pub fn list_memories(&self) -> Vec<String> {
        let mut names: Vec<_> = self.memories.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn memory_count(&self) -> usize {
        self.memories.len()
    }
}

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use abu_agent::{memory::Memory as MemoryTrait, model::ChatModel};
use abu_provider::{chat::ChatMessage, ChatProvide};
use regex::Regex;
use tokio::sync::RwLock;
use walkdir::WalkDir;

pub const MEMORY_GUIDANCE: &str = r#"## Memory system

You have a persistent, file-based memory system. Memories survive across conversations.

### How it works
- Each turn, you receive a **memory index** (name + description + kind only — no full content).
- When a memory looks relevant, call `fetch_memory` to retrieve its full content.
- Call `save_memory` to persist new memories.

### When to save memories
- User states a preference ("I like tabs", "always use pytest") -> kind: user
- User corrects you ("don't do X", "that was wrong because...") -> kind: feedback
- You learn a non-obvious project fact (compliance rules, business constraints) -> kind: project
- You learn where an external resource lives (ticket board, dashboard, docs URL) -> kind: reference

### When NOT to save
- Code patterns, conventions, file paths (derivable from code)
- Git history or recent changes (use git log)
- Debugging solutions (the fix is in the code)
- Temporary task state (current branch, open PRs)
- Secrets or credentials"#;

// ============================================================================
// SaveMemoryTool
// ============================================================================

pub struct SaveMemoryTool {
    pub memory_manager: Arc<RwLock<MemoryManager>>,
}

impl SaveMemoryTool {
    pub fn new(memory_manager: Arc<RwLock<MemoryManager>>) -> Self {
        Self { memory_manager }
    }
}

#[abu_tool::tool(
    struct_name = SaveMemoryTool,
    name = "save_memory",
    description = "Save a persistent memory that survives across conversations."
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
    let mut mgr = self.memory_manager.write().await;
    mgr.mark_save_memory_called();
    mgr.save_memory(name, description, kind, content)
}

#[abu_tool::tool(
    struct_name = FetchMemoryTool,
    name = "fetch_memory",
    description = "Retrieve the full content of a specific memory by name. Use this when a memory in the index looks relevant and you need its complete details."
)]
pub async fn fetch_memory(
    &self,
    #[arg(description = "The memory name to fetch (as shown in the memory index)")]
    name: &str,
) -> anyhow::Result<String> {
    let mgr = self.memory_manager.read().await;
    match mgr.get_memory_content(name) {
        Some(content) => Ok(content),
        None => Ok(format!("No memory found with name '{}'.", name)),
    }
}

pub struct FetchMemoryTool {
    pub memory_manager: Arc<RwLock<MemoryManager>>,
}

impl FetchMemoryTool {
    pub fn new(memory_manager: Arc<RwLock<MemoryManager>>) -> Self {
        Self { memory_manager }
    }
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

// Intermediate type for parsing LLM JSON responses in AutoMemory::add
struct MemoryExtract {
    name: String,
    description: String,
    kind: MemoryKind,
    content: String,
}

fn parse_memory_json(text: &str) -> Vec<MemoryExtract> {
    // Strip markdown code fences if present
    let text = text.trim();
    let text = if text.starts_with("```") {
        text.lines()
            .skip(1)
            .take_while(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        text.to_string()
    };

    let text = text.trim();
    if text.is_empty() || text == "[]" {
        return vec![];
    }

    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Background memory extraction JSON parse error: {}. Raw: {:.200}",
                e, text
            );
            return vec![];
        }
    };

    let arr = match &json {
        serde_json::Value::Array(arr) => arr,
        _ => {
            eprintln!("Memory extraction not a JSON array: {:.200}", text);
            return vec![];
        }
    };

    arr.iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.to_string();
            if name.is_empty() {
                return None;
            }
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let kind = item
                .get("kind")
                .and_then(|v| v.as_str())
                .map(MemoryKind::from_str)
                .unwrap_or(MemoryKind::Project);
            let content = item
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if content.is_empty() {
                return None;
            }
            Some(MemoryExtract {
                name,
                description,
                kind,
                content,
            })
        })
        .collect()
}

// ============================================================================
// MemoryManager
// ============================================================================

pub struct MemoryManager {
    pub memory_dir: PathBuf,
    pub memories: HashMap<String, Memory>,
    claude_md_parts: Vec<(PathBuf, String)>,
    save_memory_called_this_turn: bool,
}

impl MemoryManager {
    pub fn new<P: Into<PathBuf>>(memory_dir: P) -> anyhow::Result<Self> {
        let memory_dir = memory_dir.into();
        let mut manager = Self {
            memory_dir,
            memories: HashMap::new(),
            claude_md_parts: Vec::new(),
            save_memory_called_this_turn: false,
        };
        manager.load_claude_md()?;
        manager.load_all()?;
        Ok(manager)
    }

    fn load_claude_md(&mut self) -> anyhow::Result<()> {
        // User-global AGENT.md
        if let Some(home) = dirs::home_dir() {
            let user_claude = home.join(".claude").join("AGENT.md");
            if user_claude.exists() {
                let content = std::fs::read_to_string(&user_claude)?;
                self.claude_md_parts.push((user_claude, content));
            }
        }

        // Project AGENT.md — resolve from memory dir's parent project structure
        // The memory dir is at ~/.abu-code/projects/<name>/memory/
        // We need to find the actual project workdir. Check the canonical workdir.
        if let Ok(cwd) = std::env::current_dir() {
            let project_claude = cwd.join("AGENT.md");
            if project_claude.exists() {
                let content = std::fs::read_to_string(&project_claude)?;
                self.claude_md_parts.push((project_claude, content));
            }
        }

        Ok(())
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

    /// Build the memory index text (name + description only, no full content).
    /// Use `get_memory_content()` to fetch full content of a specific memory.
    pub fn build_context_text(&self) -> String {
        let mut parts: Vec<String> = vec![];

        // CLAUDE.md content (always relevant — shown in full)
        for (path, content) in &self.claude_md_parts {
            parts.push(format!(
                "## CLAUDE.md from {:?}\n\n{}",
                path, content
            ));
        }

        // Memory index: names + descriptions only (no full content)
        if !self.memories.is_empty() {
            let mut lines = vec![
                "# Memory index".to_string(),
                String::new(),
                "Use `fetch_memory` with a name below to retrieve full content.".to_string(),
                String::new(),
            ];

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

                lines.push(format!("## {}", kind.to_str()));
                for mem in typed {
                    lines.push(format!("- **{}**: {}", mem.name, mem.description));
                }
                lines.push(String::new());
            }

            parts.push(lines.join("\n"));
        }

        parts.join("\n")
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

    /// Mark that the LLM manually called save_memory this turn.
    /// Used to skip automatic extraction in AutoMemory::add().
    pub fn mark_save_memory_called(&mut self) {
        self.save_memory_called_this_turn = true;
    }

    /// Check and reset the save_memory_called flag.
    /// Returns true if save_memory was called this turn.
    pub fn take_save_memory_called(&mut self) -> bool {
        std::mem::take(&mut self.save_memory_called_this_turn)
    }

    pub fn list_memories(&self) -> Vec<String> {
        let mut names: Vec<_> = self.memories.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn memory_count(&self) -> usize {
        self.memories.len()
    }

    pub fn get_memory_content(&self, name: &str) -> Option<String> {
        self.memories.get(name).map(|mem| {
            format!(
                "## {} [{}]: {}\n\n{}",
                mem.name,
                mem.kind.to_str(),
                mem.description,
                mem.content
            )
        })
    }
}

// ============================================================================
// AutoMemory — implements abu_agent::memory::Memory trait
// ============================================================================

pub struct AutoMemory<P: ChatProvide> {
    manager: Arc<RwLock<MemoryManager>>,
    extraction_llm: Arc<ChatModel<P>>,
}

impl<P: ChatProvide> AutoMemory<P> {
    pub fn new(manager: Arc<RwLock<MemoryManager>>, extraction_llm: ChatModel<P>) -> Self {
        Self {
            manager,
            extraction_llm: Arc::new(extraction_llm),
        }
    }
}

#[async_trait::async_trait]
impl<P: ChatProvide + 'static> MemoryTrait for AutoMemory<P> {
    type Error = anyhow::Error;

    async fn add(&mut self, user_input: &str, ai_response: &str) -> Result<(), Self::Error> {
        // Skip automatic extraction if the LLM already called save_memory this turn
        if self.manager.write().await.take_save_memory_called() {
            return Ok(());
        }

        let user_input = user_input.to_string();
        let ai_response = ai_response.to_string();
        let manager = self.manager.clone();
        let llm = self.extraction_llm.clone();

        // Spawn background extraction so the user doesn't wait for it
        tokio::spawn(async move {
            let prompt = format!(
                r#"Analyze this conversation turn. Extract any non-obvious facts worth remembering for future conversations.

Return a JSON array of objects, each with:
- "name": short snake_case identifier (e.g. "prefer_tabs", "api_rate_limit")
- "description": one-line summary (used in the memory index)
- "kind": one of "user", "feedback", "project", "reference"
- "content": the full memory text

Kind guide:
- user: preferences, role, goals, knowledge of the user
- feedback: corrections to your behavior (include WHY so future instances can judge edge cases)
- project: non-obvious project facts, constraints, decisions
- reference: external resource pointers (dashboards, ticket trackers, docs URLs)

Do NOT save: code patterns, git history, debugging steps, temporary state, secrets.

Return [] if nothing is worth saving. Output ONLY the JSON array, no other text.

Conversation:
User: {}
AI: {}"#,
                user_input, ai_response
            );

            let messages = vec![
                ChatMessage::system("You are a knowledge extraction assistant. Output ONLY a valid JSON array. No surrounding text."),
                ChatMessage::user(prompt),
            ];

            let response = match llm.chat_no_tools(&messages).await {
                Ok(r) => r.message,
                Err(e) => {
                    eprintln!("Background memory extraction LLM error: {}", e);
                    return;
                }
            };

            let memories = parse_memory_json(&response.content);
            if memories.is_empty() {
                return;
            }

            let mut mgr = manager.write().await;
            for mem in &memories {
                if let Err(e) = mgr.save_memory(&mem.name, &mem.description, mem.kind, &mem.content) {
                    eprintln!("Background memory save error for '{}': {}", mem.name, e);
                }
            }
        });

        Ok(())
    }

    async fn search(&self, _query: &str) -> Result<Vec<ChatMessage>, Self::Error> {
        let ctx = self.manager.read().await.build_context_text();

        if ctx.is_empty() {
            return Ok(vec![]);
        }

        Ok(vec![ChatMessage::user(format!(
            "## Persistent context (loaded from memory)\n\n{}",
            ctx
        ))])
    }

    async fn clear(&mut self) -> Result<(), Self::Error> {
        let mut mgr = self.manager.write().await;
        mgr.memories.clear();

        if mgr.memory_dir.exists() {
            for entry in std::fs::read_dir(&mgr.memory_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "md")
                    && path.file_name().map_or(false, |n| n != "MEMORY.md")
                {
                    std::fs::remove_file(&path)?;
                }
            }
        }

        mgr.rebuild_index()?;
        Ok(())
    }
}

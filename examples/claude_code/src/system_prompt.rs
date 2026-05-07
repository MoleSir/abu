use std::path::PathBuf;

use abu_agent::middleware::{MiddlewareFlow, SystemPromptMiddleware};
use chrono::Utc;

pub struct SystemPromptBuilder {
    pub workdir: PathBuf,
}

impl SystemPromptBuilder {
    pub fn new<P: Into<PathBuf>>(workdir: P) -> Self {
        Self {
            workdir: workdir.into(),
        }
    }

    pub fn build(&self) -> anyhow::Result<String> {
        let mut sections: Vec<String> = vec![];

        sections.push(self.build_core());
        sections.push(self.build_claude_md()?);
        sections.push(self.build_dynamic_context());

        Ok(sections.join("\n\n"))
    }

    fn build_core(&self) -> String {
        format!(
            r#"You are a coding agent — a CLI tool similar to Claude Code. You operate in {:?}.

## Tone and style
- Be concise and direct. Don't narrate your thought process.
- When referencing code, use file_path:line_number format.

## Doing tasks
- Use the provided tools to explore, read, write, and edit files.
- Prefer editing existing files over creating new ones.
- Don't add features, refactor, or introduce abstractions beyond what the task requires.
- Default to writing no comments.

## Executing actions with care
- Carefully consider the reversibility and blast radius of actions.
- For destructive operations (rm -rf, git reset --hard, etc.), double-check before proceeding.
- Prefer using the Edit tool for surgical changes.

## Git safety
- NEVER update git config.
- NEVER run destructive git commands unless explicitly requested.
- NEVER skip hooks (--no-verify) unless explicitly requested.
- Always create NEW commits rather than amending unless explicitly requested.

## Task tracking
- Use task_create/task_update/task_list/task_get to plan and track your work.
- Mark tasks as in_progress before starting, completed when done.
- Create tasks for any non-trivial multi-step work."#,
            self.workdir
        )
    }

    fn build_claude_md(&self) -> anyhow::Result<String> {
        let mut sources: Vec<(PathBuf, String)> = vec![];

        // User-global CLAUDE.md
        if let Some(home) = dirs::home_dir() {
            let user_claude = home.join(".claude").join("CLAUDE.md");
            if user_claude.exists() {
                let content = std::fs::read_to_string(&user_claude)?;
                sources.push((user_claude, content));
            }
        }

        // Project CLAUDE.md
        let project_claude = self.workdir.join("CLAUDE.md");
        if project_claude.exists() {
            let content = std::fs::read_to_string(&project_claude)?;
            sources.push((project_claude, content));
        }

        if sources.is_empty() {
            return Ok(String::new());
        }

        let mut parts = vec!["# CLAUDE.md instructions".to_string()];
        for (path, content) in sources {
            parts.push(format!("## From {:?}", path));
            parts.push(content);
        }

        Ok(parts.join("\n\n"))
    }

    fn build_dynamic_context(&self) -> String {
        let mut lines = vec!["# Dynamic context".to_string()];

        lines.push(format!("Current date: {}", Utc::now().format("%Y-%m-%d")));
        lines.push(format!("Working directory: {:?}", self.workdir));
        lines.push("Platform: Linux".to_string());

        // Git status if available
        if let Ok(output) = std::process::Command::new("git")
            .args(["status", "--short"])
            .current_dir(&self.workdir)
            .output()
        {
            let status = String::from_utf8_lossy(&output.stdout);
            if !status.trim().is_empty() {
                lines.push(format!("Git status:\n{}", status));
            }
        }

        lines.join("\n")
    }
}

#[async_trait::async_trait]
impl SystemPromptMiddleware for SystemPromptBuilder {
    type Error = anyhow::Error;

    async fn intercept(&mut self, prompt: &mut String) -> Result<MiddlewareFlow, Self::Error> {
        let sys_prompt = self.build()?;
        prompt.push_str(&sys_prompt);
        Ok(MiddlewareFlow::Continue)
    }
}

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
            r#"You are Abu Code — a coding agent CLI. You operate in {:?}.

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

## TODO tracking (MANDATORY — enforced)

**CRITICAL: You MUST create a TODO list BEFORE using any mutating tools**
(write_file, edit_file, bash that modifies files, etc.). This is NOT optional.
If you use a mutating tool without active TODOs, you are violating protocol.

TODOs are batched per request. Each user request gets its own batch.
When all TODOs in a batch are completed, the batch auto-archives.

### When to create TODOs
- ANY request that involves writing, editing, or creating files
- ANY request that requires multiple steps to verify completion
- ANY request where you need to explore before implementing
- When in doubt, CREATE TODOs. There is no penalty for over-decomposition.

### How to break down work
- Break the user's request into 2-5 concrete, verifiable subtasks.
- Each TODO MUST be a single action with a clear deliverable.
- First TODO should typically be exploration/understanding ("Explore X").
- Last TODO should be verification/testing ("Verify X works").
- Use blocked_by to express dependencies — e.g. #2 blocked_by [1], #3 blocked_by [2].

### Examples

Request: "Create a fib.py and tests"
  Good TODOs:
    1: Explore existing project structure and conventions
    2: Implement fib.py with recursive fibonacci
    3: Write pytest tests in test_fib.py (blocked_by [2])
    4: Run tests and verify they pass (blocked_by [3])
  Bad: single TODO "Create fib.py and tests"

Request: "Add a /cost command"
  Good TODOs:
    1: Explore CLI command handling and CmdCtx structure
    2: Implement token counting hook
    3: Implement /cost command handler (blocked_by [1, 2])
    4: Verify /cost output is correct (blocked_by [3])
  Bad: single TODO "Add /cost command"

### During execution
- Only ONE TODO in_progress at a time. Finish it before starting the next.
- Mark TODOs completed IMMEDIATELY after finishing — this unblocks dependents.
- If a TODO needs more subtasks, create them on the fly.
- todo_list shows current batch status.
- When the last TODO is completed, the batch is done.

## Subagents
You have three subagent types, each started via their tool:
- **task** — General-purpose: read, write, edit, execute. Use for implementing features or fixing bugs.
- **explore** — Read-only code explorer: can Glob, Grep, Read, and run read-only Bash. Use for understanding code, finding definitions, searching patterns. Returns a structured report.
- **plan** — Read-only architect: explores then designs step-by-step plans with file paths and trade-offs. Use before complex implementations to avoid wasted work.

Prefer explore over task when you just need to understand code — it's faster and won't accidentally modify files.

## Background tasks
- Use background_run for long-running commands (builds, tests, installs). Returns a task ID immediately.
- Use background_check to poll a specific task's status.
- Use background_list to see all background tasks.
- You will be notified when a background task completes.

## Code exploration
- Use Glob to find files by pattern (e.g. '**/*.rs' for all Rust files).
- Use Grep to search file contents with regex (e.g. pattern: 'fn main', path_filter: '**/*.rs')."#,
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

use std::sync::Mutex;

use abu_agent::hook::{Hook, HookEvent};
use colored::Colorize;

const MAX_VISIBLE_ARGS: usize = 80;

/// Keeps track of the currently executing tool so ToolEnd can display a diff.
struct CurrentTool {
    name: String,
    arguments: String,
}

pub struct AbuHook {
    current_tool: Mutex<Option<CurrentTool>>,
}

impl AbuHook {
    pub fn new() -> Self {
        Self {
            current_tool: Mutex::new(None),
        }
    }

    /// Format tool arguments for display. Shows key=value pairs compactly.
    fn format_args(args: &str) -> String {
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(args) else {
            return args.to_string();
        };
        let Some(map) = obj.as_object() else {
            return args.to_string();
        };

        let parts: Vec<String> = map
            .iter()
            .map(|(k, v)| {
                let val = match v {
                    serde_json::Value::String(s) => {
                        if s.len() > MAX_VISIBLE_ARGS {
                            format!("{}...", &s[..MAX_VISIBLE_ARGS])
                        } else {
                            s.clone()
                        }
                    }
                    other => other.to_string(),
                };
                format!("{}: {}", k.dimmed(), val.normal())
            })
            .collect();
        parts.join("  ")
    }

    /// Format tool result output. Truncates long output with a note.
    fn format_output(out: &str, max_lines: usize) -> String {
        let out = out.trim();
        if out.is_empty() {
            return "(no output)".dimmed().to_string();
        }

        if out.starts_with("<persisted-output>") {
            let preview: String = out
                .lines()
                .filter(|l| !l.starts_with('<') && !l.starts_with("Full output"))
                .collect::<Vec<_>>()
                .join("\n");
            let short: String = preview.chars().take(1000).collect();
            return format!("{}\n  {}", "(large output — persisted to disk)".dimmed(), short.trim());
        }

        let lines: Vec<&str> = out.lines().collect();
        if lines.len() > max_lines {
            let head: String = lines[..max_lines].join("\n");
            return format!(
                "{}\n  {}",
                head,
                format!("... ({} more lines)", lines.len() - max_lines).dimmed()
            );
        }

        if out.len() > 4000 {
            let short: String = out.chars().take(4000).collect();
            return format!("{}\n  {}", short, "(truncated)".dimmed());
        }

        out.to_string()
    }

    /// Generate a colored unified-diff-like display for EditFile operations.
    fn format_diff(old: &str, new: &str) -> String {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        // Find common prefix length
        let prefix_len = old_lines
            .iter()
            .zip(new_lines.iter())
            .take_while(|(a, b)| a == b)
            .count();

        // Find common suffix length
        let suffix_len = old_lines
            .iter()
            .rev()
            .zip(new_lines.iter().rev())
            .take_while(|(a, b)| a == b)
            .count();

        let old_changed = old_lines.len().saturating_sub(prefix_len + suffix_len);
        let new_changed = new_lines.len().saturating_sub(prefix_len + suffix_len);

        let mut out = Vec::new();

        // Context before changes
        if prefix_len > 0 {
            let show_prefix = prefix_len.min(2);
            if prefix_len > show_prefix {
                out.push(format!("  {}", "... ({} lines)".dimmed()));
            }
            for line in &old_lines[prefix_len - show_prefix..prefix_len] {
                out.push(format!("  {}", line.dimmed()));
            }
        }

        // Removed lines
        for line in &old_lines[prefix_len..old_lines.len() - suffix_len] {
            out.push(format!("{} {}", "-".red().bold(), line.red()));
        }

        // Added lines
        for line in &new_lines[prefix_len..new_lines.len() - suffix_len] {
            out.push(format!("{} {}", "+".green().bold(), line.green()));
        }

        // Context after changes
        if suffix_len > 0 {
            let show_suffix = suffix_len.min(2);
            for line in &new_lines[new_lines.len() - suffix_len..new_lines.len() - suffix_len + show_suffix] {
                out.push(format!("  {}", line.dimmed()));
            }
            if suffix_len > show_suffix {
                out.push(format!("  {}", "... ({} lines)".dimmed()));
            }
        }

        if out.is_empty() {
            return "  (no changes)".dimmed().to_string();
        }

        let added = new_changed;
        let removed = old_changed;
        out.push(format!(
            "  {} removed, {} added",
            removed.to_string().red(),
            added.to_string().green()
        ));
        out.join("\n")
    }

    /// Try to generate a diff if the tool call looks like an edit_file.
    fn maybe_diff(tool_name: &str, arguments: &str, _result_context: &str) -> Option<String> {
        if tool_name != "edit_file" {
            return None;
        }
        let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
        let old = args.get("old_string")?.as_str()?;
        let new = args.get("new_string")?.as_str()?;
        if old == new {
            return Some("  (identical replacement)".dimmed().to_string());
        }
        Some(Self::format_diff(old, new))
    }
}

#[async_trait::async_trait]
impl Hook for AbuHook {
    type Error = std::convert::Infallible;

    async fn on_event(&self, event: &HookEvent<'_>) -> Result<(), Self::Error> {
        match event {
            // ===== Step boundary =====
            HookEvent::AgentStepStart { step } => {
                let width = term_width().min(60);
                let label = format!(" Step {} ", step);
                let fill = width.saturating_sub(label.len());
                let line: String = std::iter::repeat('─').take(fill).collect();
                println!("\n{}", format!("{}{}", label.bold().white(), line.dimmed()));
            }

            // ===== Agent start =====
            HookEvent::AgentStart { query } => {
                let short: String = query.chars().take(120).collect();
                let display = if query.len() > 120 {
                    format!("{}...", short)
                } else {
                    short
                };
                println!("{}", format!("  working on: {}", display.dimmed()).dimmed());
            }

            // ===== Agent end =====
            HookEvent::AgentEnd { .. } => {
                println!();
            }

            // ===== LLM response =====
            HookEvent::LlmEnd { message, .. } => {
                let has_tools = !message.tool_calls.is_empty();
                let has_content = !message.content.is_empty();

                if has_content && has_tools {
                    // Thinking out loud before calling tools — dim it
                    for line in message.content.lines() {
                        println!("  {}", line.dimmed());
                    }
                } else if has_content && !has_tools {
                    // Final answer — print with light formatting
                    println!();
                    // Simple markdown-ish rendering: treat ``` fences as code blocks
                    let mut in_code = false;
                    for line in message.content.lines() {
                        if line.starts_with("```") {
                            in_code = !in_code;
                            continue;
                        }
                        if in_code {
                            println!("  {}", line.dimmed());
                        } else if line.starts_with("# ") {
                            println!("{}", line.bold().white());
                        } else if line.starts_with("## ") {
                            println!("{}", line.bold().white());
                        } else if line.starts_with("- ") || line.starts_with("* ") {
                            println!("  {}", line);
                        } else if line.starts_with("| ") {
                            // Table — dim it slightly
                            println!("  {}", line.dimmed());
                        } else {
                            println!("{}", line);
                        }
                    }
                    println!();
                }
                // If only tool calls with no text content, ToolStart handles display
            }

            // ===== Tool start =====
            HookEvent::ToolStart { tool_call, .. } => {
                let args = Self::format_args(&tool_call.arguments);
                let icon = "◆".truecolor(100, 160, 220);
                let name = tool_call.name.bold().white();
                if args.is_empty() {
                    println!("  {} {}", icon, name);
                } else {
                    println!("  {} {}  {}", icon, name, args);
                }
                // Remember tool info for ToolEnd's diff display
                *self.current_tool.lock().unwrap() = Some(CurrentTool {
                    name: tool_call.name.clone(),
                    arguments: tool_call.arguments.clone(),
                });
            }

            // ===== Tool end =====
            HookEvent::ToolEnd { result, .. } => {
                let prefix = "│".truecolor(80, 80, 80);
                let current = self.current_tool.lock().unwrap().take();

                if result.is_error {
                    for line in result.context.lines() {
                        println!("  {} {}", prefix, line.red());
                    }
                } else {
                    // Show diff for edit_file
                    let mut showed_diff = false;
                    if let Some(ref tool) = current {
                        if let Some(diff) =
                            Self::maybe_diff(&tool.name, &tool.arguments, &result.context)
                        {
                            for line in diff.lines() {
                                println!("  {} {}", prefix, line);
                            }
                            showed_diff = true;
                        }
                    }

                    if !showed_diff {
                        let output = Self::format_output(&result.context, 15);
                        if output.lines().count() == 1 {
                            println!("  {} {}", prefix, output.trim().dimmed());
                        } else {
                            for line in output.lines() {
                                println!("  {} {}", prefix, line);
                            }
                        }
                    }
                }
            }

            // ===== Tool error =====
            HookEvent::ToolError { context, .. } => {
                let prefix = "│".truecolor(80, 80, 80);
                println!("  {} {}", prefix, format!("Error: {}", context).red().bold());
            }

            // ===== Max iterations =====
            HookEvent::AgentMaxIteration => {
                println!(
                    "\n  {}",
                    "Max iterations reached — stopping.".yellow().bold()
                );
            }

            // Silent events
            _ => {}
        }

        Ok(())
    }
}

/// Get terminal width, defaulting to 80.
fn term_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(80)
}

// ============================================================================
// Silent hook for subagents
// ============================================================================

pub struct SilentHook;

impl SilentHook {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl Hook for SilentHook {
    type Error = std::convert::Infallible;
    async fn on_event(&self, _event: &HookEvent<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
}

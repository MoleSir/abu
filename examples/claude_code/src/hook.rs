use abu_agent::hook::{Hook, HookEvent};
use colored::Colorize;

pub struct ClaudeCodeHook;

impl ClaudeCodeHook {
    pub fn new() -> Self {
        Self
    }

    fn format_args(args: &str) -> String {
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(args) {
            if let Some(map) = obj.as_object() {
                let parts: Vec<String> = map
                    .iter()
                    .map(|(k, v)| match v {
                        serde_json::Value::String(s) => {
                            let display = if s.len() > 80 {
                                format!("{}...", &s[..80])
                            } else {
                                s.clone()
                            };
                            format!("{}: {}", k.dimmed(), display.normal())
                        }
                        other => format!("{}: {}", k.dimmed(), other),
                    })
                    .collect();
                return parts.join(", ");
            }
        }
        args.to_string()
    }

    fn format_output(out: &str) -> String {
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
            return format!("{}\n  {}", "(large output)".dimmed(), short.trim());
        }

        let lines: Vec<&str> = out.lines().collect();
        if lines.len() > 30 {
            let head: String = lines[..20].join("\n");
            return format!(
                "{}\n  {}",
                head,
                format!("... ({} more lines)", lines.len() - 20).dimmed()
            );
        }

        if out.len() > 2000 {
            let short: String = out.chars().take(2000).collect();
            return format!("{}\n  {}", short, "(truncated)".dimmed());
        }

        out.to_string()
    }
}

#[async_trait::async_trait]
impl Hook for ClaudeCodeHook {
    type Error = std::convert::Infallible;

    async fn on_event(&self, event: &HookEvent<'_>) -> Result<(), Self::Error> {
        match event {
            HookEvent::LlmEnd {
                step: _,
                message,
            } => {
                let has_tools = !message.tool_calls.is_empty();
                let has_content = !message.content.is_empty();

                if has_content && has_tools {
                    // Thinking out loud before calling tools
                    println!("{}", message.content.dimmed());
                } else if has_content && !has_tools {
                    // Final answer
                    println!("\n{}", message.content);
                }
                // If no content but has tools, ToolStart will show what's happening
            }

            HookEvent::ToolStart {
                step: _,
                tool_call,
            } => {
                let args = Self::format_args(&tool_call.arguments);
                println!(
                    "  {} {} {}",
                    "⏺".truecolor(150, 150, 150),
                    tool_call.name.bold().white(),
                    args.truecolor(120, 120, 120)
                );
            }

            HookEvent::ToolEnd {
                step: _,
                result,
            } => {
                let output = Self::format_output(&result.context);
                if result.is_error {
                    for line in output.lines() {
                        println!("    {}", line.red());
                    }
                } else {
                    for line in output.lines() {
                        println!("    {}", line.dimmed());
                    }
                }
            }

            HookEvent::ToolError {
                step: _,
                context,
            } => {
                println!("    {}", format!("Error: {}", context).red().bold());
            }

            HookEvent::AgentMaxIteration => {
                println!(
                    "\n  {}",
                    "Max iterations reached.".yellow().bold()
                );
            }

            // AgentStart, AgentEnd, Memory*, ContextBuild, StepStart/End: silent
            _ => {}
        }

        Ok(())
    }
}

// ============================================================================
// Silent hook for subagents (their output would be confusing interleaved)
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

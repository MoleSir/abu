use colored::Colorize;
use super::{Hook, HookEvent};

pub struct ConsoleLoggerHook;

impl ConsoleLoggerHook {
    pub fn new() -> Self {
        Self
    }

    // fn truncate(text: &str, max_len: usize) -> String {
    //     let text = text.replace('\n', " ");
    //     if text.chars().count() > max_len {
    //         let truncated: String = text.chars().take(max_len).collect();
    //         format!("{}...", truncated)
    //     } else {
    //         text
    //     }
    // }
}

#[async_trait::async_trait]
impl Hook for ConsoleLoggerHook {
    type Error = std::convert::Infallible;

    #[allow(unused)]
    async fn on_event(&self, event: &HookEvent<'_>) -> Result<(), Self::Error> {
        match event {
            // ==========================================
            // Agent 总体生命周期
            // ==========================================
            HookEvent::AgentStart { query } => {
                println!("\n{}", "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓".bold().blue());
                println!("┃ 🤖 {} {}", "AGENT START".bold().green(), "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bold().blue());
                println!("┃ {} {}", "Query:".bold().cyan(), query.white());
                println!("{}", "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛".bold().blue());
            }
            HookEvent::AgentEnd { result } => {
                println!("\n{}", "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓".bold().blue());
                println!("┃ 🏁 {} {}", "AGENT FINISHED".bold().green(), "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bold().blue());
                println!("┃ {}", result.green());
                println!("{}", "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛\n".bold().blue());
            }
            HookEvent::AgentMaxIteration => {
                println!("\n{}", "🛑 [WARNING] Agent halted: Reached maximum iterations!".bold().on_red().white());
            }

            // ==========================================
            // Step 生命周期
            // ==========================================
            HookEvent::AgentStepStart { step } => {
                println!("\n{}", format!("╭── [ 🔄 Step {} ] ───────────────────────────────────────────", step).bold().yellow());
            }
            HookEvent::AgentStepEnd { step, message } => {
                let status = if message.tool_calls.is_empty() {
                    "Final Answer Generated".green()
                } else {
                    format!("Requested {} Tools", message.tool_calls.len()).yellow()
                };
                println!("{}", format!("╰── [ Step {} End ] : {} ──────────────────────────────────────", step, status).bold().yellow());
            }

            // ==========================================
            // 记忆与上下文
            // ==========================================
            HookEvent::MemorySearch { query, results } => {
                let count = results.len();
                if count > 0 {
                    println!("│  🧠 {} Searched for '{}', found {} records.", "Memory:".bold().cyan(), query, count);
                } else {
                    println!("│  🧠 {} No relevant memory found.", "Memory:".bold().bright_black());
                }
            }
            HookEvent::MemoryAdd { user, assistant } => {
                println!("│  💾 {} Saved latest interaction to memory.", "Memory:".bold().cyan());
                // 仅在 Debug 时可以取消注释打印详细记忆
                // println!("│       User: {}", Self::truncate(user, 30).bright_black());
                // println!("│       AI:   {}", Self::truncate(assistant, 30).bright_black());
            }
            HookEvent::ContextBuild { query: _, messages } => {
                println!("│  🏗️  {} Assembled {} messages.", "Context:".bold().magenta(), messages.len());
            }

            // ==========================================
            // LLM 交互
            // ==========================================
            HookEvent::LlmStart { step: _, messages } => {
                println!("│  💬 {} Sending prompt ({} msgs)...", "LLM Req:".bold().blue(), messages.len());
            }
            HookEvent::LlmEnd { step: _, message } => {
                if !message.content.is_empty() {
                    println!("│  💡 {} {}", "LLM Res:".bold().blue(), message.content.white());
                }
                if !message.tool_calls.is_empty() {
                    println!("│  🛠️  {} Model decided to call {} tool(s).", "LLM Res:".bold().yellow(), message.tool_calls.len());
                }
            }

            // ==========================================
            // 工具执行
            // ==========================================
            HookEvent::ToolStart { step: _, tool_call } => {
                let args = &tool_call.arguments;
                println!("│      ▶ {} {}({})", "Executing:".bold().magenta(), tool_call.name.bold().white(), args.bright_black());
            }
            HookEvent::ToolEnd { step: _, result } => {
                let out = &result.context;
                if result.is_error {
                    println!("│      ❌ {} {}", "Tool Fail:".bold().red(), out.red());
                } else {
                    println!("│      ✅ {} {}", "Tool Done:".bold().green(), out.bright_black());
                }
            }
            HookEvent::ToolError { step: _, context } => {
                println!("│      🚨 {} {}", "Tool Fatal:".bold().red().on_black(), context.red());
            }
        }

        Ok(())
    }
}
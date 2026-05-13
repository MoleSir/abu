use std::sync::{Arc, OnceLock};
use abu_provider::deepseek::DeepSeek;
use abu_agent::{tool::ExecutionMode, Agent};
use async_trait::async_trait;
use tokio::sync::RwLock;
use crate::{
    compact::SummaryCompact, memory::{AutoMemory, MemoryManager},
    session::SessionManager,
};

pub type AppAgent = Agent<DeepSeek, AutoMemory<DeepSeek>, SummaryCompact<DeepSeek>>;

/// Shared state available to all commands (everything except the Agent).
pub struct CommandState {
    pub memory_manager: Arc<RwLock<MemoryManager>>,
    pub session_manager: Arc<RwLock<SessionManager>>,
}

pub enum CmdResult {
    /// Command was handled; continue the REPL.
    Handled,
    /// Exit the REPL.
    Exit,
}

/// Full command: metadata + async dispatch.
#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;
    fn aliases(&self) -> &'static [&'static str];
    fn description(&self) -> &'static str;
    async fn dispatch(&self, ctx: &CommandState, agent: &mut AppAgent, args: &str) -> anyhow::Result<CmdResult>;
}

/// A simple macro to easily implement the metadata methods for `Command`.
macro_rules! command_meta {
    ($name:expr, $desc:expr) => {
        fn name(&self) -> &'static str { $name }
        fn aliases(&self) -> &'static [&'static str] { &[] }
        fn description(&self) -> &'static str { $desc }
    };
    ($name:expr, $desc:expr, $aliases:expr) => {
        fn name(&self) -> &'static str { $name }
        fn aliases(&self) -> &'static [&'static str] { $aliases }
        fn description(&self) -> &'static str { $desc }
    };
}

// ============================================================================
// Commands
// ============================================================================

pub struct HelpCmd;
#[async_trait]
impl Command for HelpCmd {
    command_meta!("/help", "Show this help");

    async fn dispatch(&self, _ctx: &CommandState, _agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        println!("Commands:");
        for info in all_commands() {
            let alias_str = if info.aliases().is_empty() {
                String::new()
            } else {
                format!(" ({})", info.aliases().join(", "))
            };
            println!("  {:<10} - {}{}", info.name(), info.description(), alias_str);
        }
        Ok(CmdResult::Handled)
    }
}

pub struct ToolsCmd;
#[async_trait]
impl Command for ToolsCmd {
    command_meta!("/tools", "List all registered tools");

    async fn dispatch(&self, _ctx: &CommandState, agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        let tools = agent.tool_list();
        println!("{} tools registered:", tools.len());
        for tool in tools {
            println!("  {} — {}", tool.name, tool.description);
        }
        Ok(CmdResult::Handled)
    }
}

pub struct ModeShowCmd;
#[async_trait]
impl Command for ModeShowCmd {
    command_meta!("/mode", "Show permission execution mode");

    async fn dispatch(&self, _ctx: &CommandState, agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        let mode = agent.tools.execution_mode();
        println!("Execution mode: {:?}", mode);
        Ok(CmdResult::Handled)
    }
}

pub struct ModePlanCmd;
#[async_trait]
impl Command for ModePlanCmd {
    command_meta!("/plan", "Switch to Plan mode (read-only)");

    async fn dispatch(&self, _ctx: &CommandState, agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        agent.tools.set_execution_mode(ExecutionMode::Plan);
        println!("Switched to Plan mode (read-only).");
        Ok(CmdResult::Handled)
    }
}

pub struct ModeAutoCmd;
#[async_trait]
impl Command for ModeAutoCmd {
    command_meta!("/auto", "Switch to Auto mode (safe tools auto-approved)");

    async fn dispatch(&self, _ctx: &CommandState, agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        agent.tools.set_execution_mode(ExecutionMode::Auto);
        println!("Switched to Auto mode (safe tools auto-approved).");
        Ok(CmdResult::Handled)
    }
}

pub struct ModeDefaultCmd;
#[async_trait]
impl Command for ModeDefaultCmd {
    command_meta!("/default", "Switch to Default mode (ask for all mutating)");

    async fn dispatch(&self, _ctx: &CommandState, agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        agent.tools.set_execution_mode(ExecutionMode::Default);
        println!("Switched to Default mode (ask for all mutating tools).");
        Ok(CmdResult::Handled)
    }
}

pub struct MemoryCmd;
#[async_trait]
impl Command for MemoryCmd {
    command_meta!("/memory", "List saved memories");

    async fn dispatch(&self, ctx: &CommandState, _agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        let mm = ctx.memory_manager.read().await;
        let names = mm.list_memories();
        println!("Path: {:?}", mm.memory_dir);
        if names.is_empty() {
            println!("No memories saved.");
        } else {
            println!("Saved memories:");
            for name in names {
                println!("  - {}", name);
            }
        }
        Ok(CmdResult::Handled)
    }
}

pub struct TodosCmd;
#[async_trait]
impl Command for TodosCmd {
    command_meta!("/todos", "List current TODOs");

    async fn dispatch(&self, ctx: &CommandState, _agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        match ctx.session_manager.read().await.list_todos().await {
            Ok(list) => {
                println!("{}", list);
                Ok(CmdResult::Handled)
            }
            Err(e) => {
                println!("Error listing tasks: {}", e);
                Ok(CmdResult::Handled)
            }
        }
    }
}

pub struct SessionsCmd;
#[async_trait]
impl Command for SessionsCmd {
    command_meta!("/sessions", "List saved sessions");

    async fn dispatch(&self, ctx: &CommandState, _agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        match ctx.session_manager.read().await.list_sessions().await {
            Ok(sessions) => {
                if sessions.is_empty() {
                    println!("No saved sessions.");
                } else {
                    println!("Saved sessions:");
                    for info in &sessions {
                        let marker = if info.is_current { " (current)" } else { "" };
                        println!("  {} - {} messages{}", info.id, info.message_count, marker);
                    }
                }
            }
            Err(e) => println!("Error listing sessions: {}", e),
        }
        Ok(CmdResult::Handled)
    }
}

pub struct ClearCmd;
#[async_trait]
impl Command for ClearCmd {
    command_meta!("/clear", "Start a fresh conversation");

    async fn dispatch(&self, _ctx: &CommandState, agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        agent.conversations.clear();
        println!("Session cleared. Starting fresh.");
        Ok(CmdResult::Handled)
    }
}

pub struct SaveCmd;
#[async_trait]
impl Command for SaveCmd {
    command_meta!("/save", "Manually save current conversation");

    async fn dispatch(&self, ctx: &CommandState, agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        if agent.conversations.is_empty() {
            println!("Nothing to save.");
        } else {
            match ctx.session_manager.read().await.save_conversation(&agent.conversations).await {
                Ok(id) => println!("Session {} saved ({} messages).", id, agent.conversations.len()),
                Err(e) => println!("Failed to save session: {}", e),
            }
        }
        Ok(CmdResult::Handled)
    }
}

pub struct QuitCmd;
#[async_trait]
impl Command for QuitCmd {
    command_meta!("/quit", "Exit (auto-saves conversation)", &["/exit"]);

    async fn dispatch(&self, _ctx: &CommandState, _agent: &mut AppAgent, _args: &str) -> anyhow::Result<CmdResult> {
        Ok(CmdResult::Exit)
    }
}

pub struct ResumeCmd;
#[async_trait]
impl Command for ResumeCmd {
    command_meta!("/resume", "Switch to a different session (/resume <id>)");

    async fn dispatch(&self, ctx: &CommandState, agent: &mut AppAgent, args: &str) -> anyhow::Result<CmdResult> {
        match args {
            "" => {
                match ctx.session_manager.read().await.list_sessions().await {
                    Ok(sessions) => {
                        if sessions.is_empty() {
                            println!("No saved sessions.");
                        } else {
                            println!("Available sessions:");
                            for info in &sessions {
                                let marker = if info.is_current { " (current)" } else { "" };
                                println!("  {} - {} messages{}", info.id, info.message_count, marker);
                            }
                            println!("Usage: /resume <id>");
                        }
                    }
                    Err(e) => println!("Error listing sessions: {}", e),
                }
            }
            id => {
                let current = ctx.session_manager.read().await.current_session_id();
                if id == &current {
                    println!("Already on session {}.", id);
                    return Ok(CmdResult::Handled);
                }

                match ctx.session_manager.write().await.switch_session(id, &agent.conversations).await {
                    Ok(conversations) => {
                        let count = conversations.len();
                        agent.compact.restore_state(&conversations);
                        agent.conversations = conversations;
                        println!("Switched to session {} ({} messages).", id, count);
                    }
                    Err(e) => println!("Failed to switch to session {}: {:?}", id, e),
                }
            }
        }
        Ok(CmdResult::Handled)
    }
}

// ============================================================================
// Registry
// ============================================================================

static COMMAND_PARSERS: OnceLock<Vec<Box<dyn Command>>> = OnceLock::new();

pub fn all_commands() -> &'static Vec<Box<dyn Command>> {
    COMMAND_PARSERS.get_or_init(|| {
        vec![
            Box::new(HelpCmd),
            Box::new(ToolsCmd),
            Box::new(ModeShowCmd),
            Box::new(ModePlanCmd),
            Box::new(ModeAutoCmd),
            Box::new(ModeDefaultCmd),
            Box::new(MemoryCmd),
            Box::new(TodosCmd),
            Box::new(SessionsCmd),
            Box::new(ClearCmd),
            Box::new(ResumeCmd),
            Box::new(SaveCmd),
            Box::new(QuitCmd),
        ]
    })
}

pub async fn execute_command(ctx: &CommandState, agent: &mut AppAgent, input: &str) -> anyhow::Result<CmdResult> {
    let cmd_name = input.split_whitespace().next().expect("empty command").trim();
    let cmd_args = input[cmd_name.len()..].trim();

    for cmd in all_commands() {
        if cmd.name() == cmd_name {
            return cmd.dispatch(ctx, agent, cmd_args).await;
        }
    }
    eprintln!("Unkown command: {}", input);
    Ok(CmdResult::Handled)
}

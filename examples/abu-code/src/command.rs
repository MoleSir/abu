//! Slash-command system.
//!
//! Each command is a variant of the [`Command`] enum. The registry is
//! generated from `Command::all()` which drives help text and tab completion.

use std::{path::PathBuf, sync::Arc};

use abu_agent::tool::ExecutionMode;
use tokio::sync::RwLock;

use crate::{
    memory::MemoryManager,
    session::SessionManager,
    todo::TodoManager,
};

// ============================================================================
// Context
// ============================================================================

/// Shared state available to all commands (everything except the Agent, which
/// has complex generic bounds and is passed separately).
pub struct CmdCtx {
    pub memory_manager: Arc<RwLock<MemoryManager>>,
    pub todo_manager: Arc<std::sync::Mutex<TodoManager>>,
    pub session_manager: Arc<SessionManager>,
    #[allow(dead_code)]
    pub data_dir: PathBuf,
}

// ============================================================================
// Result
// ============================================================================

pub enum CmdResult {
    /// Command was handled; continue the REPL.
    Handled,
    /// Exit the REPL.
    Exit,
}

// ============================================================================
// Command enum
// ============================================================================

pub enum Command {
    Help,
    Tools,
    ModeShow,
    ModePlan,
    ModeAuto,
    ModeDefault,
    Memory,
    Todos,
    Sessions,
    Clear,
    Save,
    Quit,
}

/// Metadata for a single command.
pub struct CommandInfo {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
}

impl Command {
    /// Parse a slash-command input. Returns `None` if the input is not a
    /// recognized command (i.e. should be passed to the agent).
    pub fn from_input(input: &str) -> Option<Self> {
        let cmd = match input {
            "/help" => Self::Help,
            "/tools" => Self::Tools,
            "/mode" => Self::ModeShow,
            "/plan" => Self::ModePlan,
            "/auto" => Self::ModeAuto,
            "/default" => Self::ModeDefault,
            "/memory" => Self::Memory,
            "/todos" => Self::Todos,
            "/sessions" => Self::Sessions,
            "/clear" => Self::Clear,
            "/save" => Self::Save,
            "/quit" | "/exit" => Self::Quit,
            _ => return None,
        };
        Some(cmd)
    }

    /// Metadata for a single command variant.
    pub fn info(&self) -> CommandInfo {
        match self {
            Self::Help => CommandInfo {
                name: "/help",
                aliases: &[],
                description: "Show this help",
            },
            Self::Tools => CommandInfo {
                name: "/tools",
                aliases: &[],
                description: "List all registered tools",
            },
            Self::ModeShow => CommandInfo {
                name: "/mode",
                aliases: &[],
                description: "Show permission execution mode",
            },
            Self::ModePlan => CommandInfo {
                name: "/plan",
                aliases: &[],
                description: "Switch to Plan mode (read-only)",
            },
            Self::ModeAuto => CommandInfo {
                name: "/auto",
                aliases: &[],
                description: "Switch to Auto mode (safe tools auto-approved)",
            },
            Self::ModeDefault => CommandInfo {
                name: "/default",
                aliases: &[],
                description: "Switch to Default mode (ask for all mutating)",
            },
            Self::Memory => CommandInfo {
                name: "/memory",
                aliases: &[],
                description: "List saved memories",
            },
            Self::Todos => CommandInfo {
                name: "/todos",
                aliases: &[],
                description: "List current TODOs",
            },
            Self::Sessions => CommandInfo {
                name: "/sessions",
                aliases: &[],
                description: "List saved sessions",
            },
            Self::Clear => CommandInfo {
                name: "/clear",
                aliases: &[],
                description: "Start a fresh session",
            },
            Self::Save => CommandInfo {
                name: "/save",
                aliases: &[],
                description: "Manually save current session",
            },
            Self::Quit => CommandInfo {
                name: "/quit",
                aliases: &["/exit"],
                description: "Exit (auto-saves session)",
            },
        }
    }

    /// Every registered command, for help text and tab completion.
    pub fn all() -> Vec<CommandInfo> {
        use Command::*;
        [
            Help, Tools, ModeShow, ModePlan, ModeAuto, ModeDefault,
            Memory, Todos, Sessions, Clear, Save, Quit,
        ]
        .iter()
        .map(|c| c.info())
        .collect()
    }

    /// Dispatch and execute this command.
    ///
    /// The `agent` parameter carries the complex generic types; only
    /// commands that need agent access use it.
    pub async fn dispatch<AgentType>(
        self,
        ctx: &CmdCtx,
        agent: &mut AgentType,
    ) -> anyhow::Result<CmdResult>
    where
        AgentType: AgentAccess,
    {
        match self {
            Self::Help => {
                println!("Commands:");
                for info in Command::all() {
                    let alias_str = if info.aliases.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", info.aliases.join(", "))
                    };
                    println!("  {:<10} - {}{}", info.name, info.description, alias_str);
                }
                Ok(CmdResult::Handled)
            }
            Self::Tools => {
                let tools = agent.tool_list();
                println!("{} tools registered:", tools.len());
                for tool in tools {
                    println!("  {} — {}", tool.name, tool.description);
                }
                Ok(CmdResult::Handled)
            }
            Self::ModeShow => {
                let mode = agent.execution_mode();
                println!("Execution mode: {:?}", mode);
                Ok(CmdResult::Handled)
            }
            Self::ModePlan => {
                agent.set_execution_mode(ExecutionMode::Plan);
                println!("Switched to Plan mode (read-only tools only).");
                Ok(CmdResult::Handled)
            }
            Self::ModeAuto => {
                agent.set_execution_mode(ExecutionMode::Auto);
                println!("Switched to Auto mode (safe tools auto-approved).");
                Ok(CmdResult::Handled)
            }
            Self::ModeDefault => {
                agent.set_execution_mode(ExecutionMode::Default);
                println!("Switched to Default mode (ask for all mutating tools).");
                Ok(CmdResult::Handled)
            }
            Self::Memory => {
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
            Self::Todos => match ctx.todo_manager.lock().unwrap().list_all() {
                Ok(list) => {
                    println!("{}", list);
                    Ok(CmdResult::Handled)
                }
                Err(e) => {
                    println!("Error listing tasks: {}", e);
                    Ok(CmdResult::Handled)
                }
            },
            Self::Sessions => {
                match ctx.session_manager.list_sessions() {
                    Ok(sessions) => {
                        if sessions.is_empty() {
                            println!("No saved sessions.");
                        } else {
                            println!("Saved sessions:");
                            for (i, path) in sessions.iter().enumerate() {
                                let name = path.file_name().unwrap().to_string_lossy();
                                let count =
                                    ctx.session_manager.count_messages(path).unwrap_or(0);
                                let marker = if i == sessions.len() - 1 {
                                    " (latest)"
                                } else {
                                    ""
                                };
                                println!("  {} - {} messages{}", name, count, marker);
                            }
                        }
                    }
                    Err(e) => println!("Error listing sessions: {}", e),
                }
                Ok(CmdResult::Handled)
            }
            Self::Clear => {
                agent.clear_session();
                println!("Session cleared. Starting fresh.");
                Ok(CmdResult::Handled)
            }
            Self::Save => {
                if agent.session_len() == 0 {
                    println!("Nothing to save.");
                } else {
                    match agent.save_session(&ctx.session_manager) {
                        Ok(path) => println!(
                            "Session saved to {:?} ({} messages)",
                            path,
                            agent.session_len()
                        ),
                        Err(e) => println!("Failed to save session: {}", e),
                    }
                }
                Ok(CmdResult::Handled)
            }
            Self::Quit => Ok(CmdResult::Exit),
        }
    }
}

// ============================================================================
// AgentAccess — abstracts the agent operations that commands need
// ============================================================================

pub trait AgentAccess {
    fn set_execution_mode(&mut self, mode: ExecutionMode);
    fn execution_mode(&self) -> Option<ExecutionMode>;
    fn tool_list(&self) -> Vec<ToolInfo>;
    fn clear_session(&mut self);
    fn session_len(&self) -> usize;
    fn save_session(
        &self,
        session_manager: &Arc<SessionManager>,
    ) -> anyhow::Result<PathBuf>;
}

/// Lightweight tool info for listing.
pub struct ToolInfo {
    pub name: String,
    pub description: String,
}

// ============================================================================
// AgentAccess impl for the concrete Agent type
// ============================================================================

impl<P, M, C> AgentAccess for abu_agent::Agent<P, M, C>
where
    P: abu_provider::ChatProvide,
    M: abu_agent::memory::Memory,
    C: abu_agent::compact::ContextCompact,
{
    fn set_execution_mode(&mut self, mode: ExecutionMode) {
        self.tools.set_execution_mode(mode);
    }

    fn execution_mode(&self) -> Option<ExecutionMode> {
        self.tools.execution_mode()
    }

    fn tool_list(&self) -> Vec<ToolInfo> {
        self.tool_list()
            .iter()
            .map(|t| ToolInfo {
                name: t.name.clone(),
                description: t.description.clone(),
            })
            .collect()
    }

    fn clear_session(&mut self) {
        self.session.clear();
    }

    fn session_len(&self) -> usize {
        self.session.len()
    }

    fn save_session(
        &self,
        session_manager: &Arc<SessionManager>,
    ) -> anyhow::Result<PathBuf> {
        session_manager.save(&self.session)
    }
}

mod background;
mod command;
mod compact;
mod config;
mod hook;
mod input;
mod memory;
mod permission;
mod session;
mod subagent;
mod system_prompt;
mod todo;
mod tools;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Context;

use abu_agent::{
    tool::ExecutionMode,
    AgentBuilder,
};
use colored::Colorize;
use tokio::sync::RwLock;

use background::{
    BackgroundCheckTool, BackgroundListTool, BackgroundManager, BackgroundMiddleware,
    BackgroundRunTool,
};
use command::{CmdCtx, CmdResult, Command};
use compact::{CompactToolResult, IncrementalCompact};
use config::{create_chat_model, create_compact_model, model_name};
use hook::AbuHook;
use memory::{MemoryManager, MemoryMiddleware, MemoryTool};
use permission::build_permission;
use session::SessionManager;
use system_prompt::SystemPromptBuilder;
use todo::{TodoCreateTool, TodoGetTool, TodoListTool, TodoManager, TodoMiddleware, TodoUpdateTool};
use tools::{init_workdir, Bash, EditFile, Glob, Grep, ReadFile, WriteFile};

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    }
}

async fn result_main() -> anyhow::Result<()> {
    dotenv::from_filename(".env").ok();

    let workdir = std::env::current_dir()?.canonicalize()?;
    init_workdir(workdir.clone());

    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;

    // Compute project dir name: <path-slug>-<short-hash>
    let path_str = workdir.to_string_lossy();
    let path_slug = path_str.trim_start_matches('/').replace('/', "-");
    let mut hasher = DefaultHasher::new();
    path_str.hash(&mut hasher);
    let project_dir_name = format!("{}-{:08x}", path_slug, hasher.finish());

    // Persistent storage under ~/.abu-code/projects/<name>/
    //   memory/     — permanent, always loaded (extracted user/preference memories)
    //   session/    — per-conversation state (resumable or resettable)
    let data_dir = home
        .join(".abu-code")
        .join("projects")
        .join(&project_dir_name);
    let memory_dir = data_dir.join("memory");
    let session_dir = data_dir.join("session");
    let todos_dir = session_dir.join("todos");
    let tool_results_dir = session_dir.join("tool_results");
    let conversations_dir = session_dir.join("conversations");
    let background_dir = session_dir.join("background");

    // Managers
    let memory_manager = Arc::new(RwLock::new(MemoryManager::new(&memory_dir).context("Failed to initialize memory manager")?));
    let todo_manager = Arc::new(std::sync::Mutex::new(TodoManager::new(&todos_dir)?));
    let session_manager = Arc::new(SessionManager::new(&conversations_dir)?);
    let background_manager = Arc::new(RwLock::new(BackgroundManager::new(&background_dir)?));

    // System prompt
    let prompt_builder = SystemPromptBuilder::new(workdir.clone());

    // LLM — configured via CHAT_MODEL env var (default: deepseek-chat)
    // Set DEEPSEEK_BASE_URL to use any OpenAI-compatible API endpoint.
    let model = create_chat_model()?;
    let compact_model = create_compact_model()?;

    // Context compaction
    let compact = IncrementalCompact::new(compact_model);
    let tool_result_compactor = CompactToolResult::new(&tool_results_dir)?;

    // Permission — rules persisted to data_dir/permissions.json
    let permission = build_permission(&data_dir);

    // Subagents — share one model instance across all three types
    let task_subagent = subagent::build_task_subagent(create_chat_model()?).await?;
    let explore_subagent = subagent::build_explore_subagent(create_chat_model()?).await?;
    let plan_subagent = subagent::build_plan_subagent(create_chat_model()?).await?;

    // Build the agent
    let mut agent = AgentBuilder::new(model)
        .system_prompt("")
        .max_iteration(25)
        .with_system_prompt_middleware(TodoMiddleware::new(todo_manager.clone()))
        .with_system_prompt_middleware(prompt_builder)
        .with_system_prompt_middleware(MemoryMiddleware::new(memory_manager.clone()))
        .with_hook(AbuHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool(EditFile::new())
        .with_tool(Glob::new())
        .with_tool(Grep::new())
        .with_tool(TodoCreateTool::new(todo_manager.clone()))
        .with_tool(TodoUpdateTool::new(todo_manager.clone()))
        .with_tool(TodoListTool::new(todo_manager.clone()))
        .with_tool(TodoGetTool::new(todo_manager.clone()))
        .with_tool(MemoryTool::new(memory_manager.clone()))
        .with_tool(BackgroundRunTool::new(background_manager.clone()))
        .with_tool(BackgroundCheckTool::new(background_manager.clone()))
        .with_tool(BackgroundListTool::new(background_manager.clone()))
        .with_subagent(task_subagent)
        .with_subagent(explore_subagent)
        .with_subagent(plan_subagent)
        .compact(compact)
        .with_llm_input_middleware(BackgroundMiddleware::new(background_manager.clone()))
        .with_tool_result_middleware(tool_result_compactor)
        .with_skills_if_exists("./skills")
        .with_mcpconfig_if_exists(&[
            workdir.join(".mcp.json"),
            home.join(".abu-code").join("mcp.json"),
        ])
        .with_permission(permission)
        .build()
        .await?;

    // Command context
    let cmd_ctx = CmdCtx {
        memory_manager: memory_manager.clone(),
        todo_manager: todo_manager.clone(),
        session_manager: session_manager.clone(),
        data_dir: data_dir.clone(),
    };

    // Create readline editor with history
    let mut editor = input::create_editor(&data_dir)?;

    // ── Project header ────────────────────────────────────────────────
    println!("Abu Code  |  Model: {}", model_name());
    println!("Project:   {:?}", workdir);

    // ── Memory (permanent, always loaded) ─────────────────────────────
    let memory_count = memory_manager.read().await.memory_count();
    if memory_count > 0 {
        println!(
            "{}",
            format!("Memory:     {} entr{} loaded", memory_count, if memory_count == 1 { "y" } else { "ies" }).dimmed()
        );
    }

    // ── Session state (resumable) ─────────────────────────────────────
    let has_todos = todo_manager.lock().unwrap().has_any_state();
    let has_bg = background_manager.read().await.any_tasks();
    let has_conversation = session_manager.has_any_state();

    let has_session_state = has_todos || has_bg || has_conversation;

    if has_session_state {
        println!();
        println!("{}", "Session state found:".dimmed());

        if has_todos {
            let mgr = todo_manager.lock().unwrap();
            let (pending, in_progress, completed) = mgr.todo_counts();
            let bid = mgr.batch_id().unwrap_or("?");
            println!(
                "  TODOs:         batch {} — {} pending, {} in progress, {} completed",
                bid, pending, in_progress, completed
            );
        }
        if has_bg {
            let bg = background_manager.read().await;
            println!(
                "  Background:    {} task(s) ({})",
                bg.len(),
                if bg.completed_count() == bg.len() {
                    "all completed".to_string()
                } else {
                    format!("{} completed, {} incomplete", bg.completed_count(), bg.len() - bg.completed_count())
                }
            );
        }
        if has_conversation {
            if let Ok(Some(latest)) = session_manager.latest_session() {
                if let Ok(count) = session_manager.count_messages(&latest) {
                    let name = latest.file_name().unwrap().to_string_lossy();
                    println!("  Conversation:  {} ({} messages)", name, count);
                }
            }
        }

        println!();
        let answer = editor
            .readline("Resume session? [Y/n]: ")
            .unwrap_or_else(|_| String::new());

        if answer.trim().to_lowercase() == "n" {
            // Discard all session state
            println!("Starting fresh session...");
            todo_manager.lock().unwrap().reset()?;
            background_manager.write().await.clear().context("Failed to clear background tasks")?;
            session_manager.clear_all()?;
        } else {
            // Resume conversation (TODOs and background already loaded)
            if let Ok(Some(latest)) = session_manager.latest_session() {
                match session_manager.load(&latest) {
                    Ok(messages) if !messages.is_empty() => {
                        agent.compact.restore_state(&messages);
                        agent.session = messages;
                        println!("Resumed {} messages.", agent.session.len());
                    }
                    _ => {}
                }
            }
        }
    }

    println!(
        "Mode:      {:?}",
        agent.tools.execution_mode().unwrap_or(ExecutionMode::Default)
    );
    println!("{}", "Type /help for commands.".dimmed());

    // REPL loop with readline
    loop {
        let query = match editor.readline("> ") {
            Ok(line) => line.trim().to_string(),
            Err(rustyline::error::ReadlineError::Interrupted) => {
                // Ctrl-C — cancel current input
                println!("^C");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                // Ctrl-D — quit
                println!();
                break;
            }
            Err(e) => {
                eprintln!("Readline error: {}", e);
                break;
            }
        };

        if query.is_empty() {
            continue;
        }

        // Add to history (skip if same as most recent entry)
        if let Err(e) = editor.add_history_entry(&query) {
            eprintln!("Failed to save history entry: {}", e);
        }

        // Dispatch slash commands through the registry
        if let Some(cmd) = Command::from_input(&query) {
            match cmd.dispatch(&cmd_ctx, &mut agent).await {
                Ok(CmdResult::Handled) => {}
                Ok(CmdResult::Exit) => {
                    if !agent.session.is_empty() {
                        if let Err(e) = session_manager.save(&agent.session) {
                            eprintln!("Failed to save session on exit: {}", e);
                        }
                    }
                    println!("Bye!");
                    break;
                }
                Err(e) => {
                    println!("Error: {:?}", e);
                }
            }
            continue;
        }

        // Run the agent
        if let Err(e) = agent.run(&query).await {
            eprintln!("Error: {:?}", e);
        }

        // Auto-save after each run
        if !agent.session.is_empty() {
            if let Err(e) = session_manager.save(&agent.session) {
                eprintln!("Failed to auto-save session: {}", e);
            }
        }
    }

    // Save history before exit
    if let Err(e) = input::save_history(&mut editor, &data_dir) {
        eprintln!("Failed to save history: {}", e);
    }

    Ok(())
}

// ============================================================================
// Builder extensions
// ============================================================================

/// Extension trait for AgentBuilder to add optional features without breaking the chain.
trait AgentBuilderExt {
    fn with_skills_if_exists(self, path: &str) -> Self;
    fn with_mcpconfig_if_exists(self, paths: &[PathBuf]) -> Self;
}

impl<P: abu_provider::ChatProvide, M: abu_agent::memory::Memory, C: abu_agent::compact::ContextCompact>
    AgentBuilderExt for AgentBuilder<P, M, C>
{
    fn with_skills_if_exists(mut self, path: &str) -> Self {
        if std::path::Path::new(path).exists() {
            self = self.with_skills(path);
        }
        self
    }

    fn with_mcpconfig_if_exists(mut self, paths: &[PathBuf]) -> Self {
        for p in paths {
            if p.exists() {
                self = self.with_mcpconfig(p.as_path());
                break;
            }
        }
        self
    }
}

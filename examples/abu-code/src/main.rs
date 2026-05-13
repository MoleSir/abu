mod compact;
mod config;
mod ui;
mod memory;
mod session;
mod prompt;
mod tools;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
};

use abu_provider::deepseek::DeepSeek;
use abu_agent::{tool::ExecutionMode, AgentBuilder,};
use colored::Colorize;
use tokio::sync::RwLock;
use compact::{CompactToolResult, SummaryCompact};
use memory::{AutoMemory, FetchMemoryTool, MemoryManager, SaveMemoryTool};
use config::{create_chat_model, create_compact_model, model_name};
use ui::hook::AbuHook;
use ui::command::{CommandState, CmdResult};
use session::SessionManager;
use session::wrap::{BackgroundCheckTool, BackgroundListTool, BackgroundMiddleware, BackgroundRunTool};
use session::wrap::{TodoCreateTool, TodoGetTool, TodoListTool, TodoMiddleware, TodoUpdateTool};
use prompt::system::SystemPromptBuilder;
use tools::{init_workdir, Bash, EditFile, Glob, Grep, ReadFile, WriteFile, web::{WebFetch, WebSearch}};
use tools::permission::build_permission;

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
    //   memory/        — permanent, always loaded (extracted user/preference memories)
    //   sessions/      — per-session directories (each with conversation.jsonl, todos/, background/)
    //   tool_results/  — tool results
    let data_dir = home
        .join(".abu-code")
        .join("projects")
        .join(&project_dir_name);
    let memory_dir = data_dir.join("memory");
    let sessions_dir = data_dir.join("sessions");
    let compact_dir = data_dir.join("tool_results");
    let permission_dir = data_dir.join("permissions.json");

    // Managers — mirrors on-disk layout: memory/ and sessions/
    // SessionManager creates a new session on construction and owns
    let session_manager = Arc::new(RwLock::new(SessionManager::new(&sessions_dir)?));
    let memory_manager = Arc::new(RwLock::new(MemoryManager::new(&memory_dir)?));

    // System prompt
    let prompt_builder = SystemPromptBuilder::new(workdir.clone());

    // Uses a separate LLM call for automatic fact extraction after each query.
    let extraction_model = create_chat_model()?;
    let abu_memory = AutoMemory::new(memory_manager.clone(), extraction_model);

    // Context compaction
    let compact_model = create_compact_model()?;
    let compact = SummaryCompact::new(compact_model);
    let tool_result_compactor = CompactToolResult::new(compact_dir).await?;

    // Permission — rules persisted to data_dir/permissions.json
    let permission = build_permission(&permission_dir);

    // Subagents — share one model instance across all three types
    let task_subagent = tools::subagent::build_task_subagent(create_chat_model()?).await?;
    let explore_subagent = tools::subagent::build_explore_subagent(create_chat_model()?).await?;
    let plan_subagent = tools::subagent::build_plan_subagent(create_chat_model()?).await?;

    // Build the agent
    let model = create_chat_model()?;
    let mut agent: abu_agent::Agent<DeepSeek, AutoMemory<DeepSeek>, SummaryCompact<DeepSeek>> = AgentBuilder::new(model)
        .memory(abu_memory)
        .system_prompt("")
        .max_iteration(25)
        .with_system_prompt_middleware(prompt_builder)
        .with_system_prompt_middleware(TodoMiddleware::new(session_manager.clone()))
        .with_hook(AbuHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool(EditFile::new())
        .with_tool(Glob::new())
        .with_tool(Grep::new())
        .with_tool(WebFetch::new())
        .with_tool(WebSearch::new())
        .with_tool(TodoCreateTool::new(session_manager.clone()))
        .with_tool(TodoUpdateTool::new(session_manager.clone()))
        .with_tool(TodoListTool::new(session_manager.clone()))
        .with_tool(TodoGetTool::new(session_manager.clone()))
        .with_tool(SaveMemoryTool::new(memory_manager.clone()))
        .with_tool(FetchMemoryTool::new(memory_manager.clone()))
        .with_tool(BackgroundRunTool::new(session_manager.clone()))
        .with_tool(BackgroundCheckTool::new(session_manager.clone()))
        .with_tool(BackgroundListTool::new(session_manager.clone()))
        .with_subagent(task_subagent)
        .with_subagent(explore_subagent)
        .with_subagent(plan_subagent)
        .compact(compact)
        .with_llm_input_middleware(BackgroundMiddleware::new(session_manager.clone()))
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
    let cmd_ctx = CommandState {
        memory_manager: memory_manager.clone(),
        session_manager: session_manager.clone(),
    };

    // Create readline editor with history
    let mut editor = ui::input::create_editor(&data_dir)?;

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

    // ── Session state ─────────────────────────────────────────────────
    let current_session = session_manager.read().await.current_session_id();
    let all_sessions = session_manager.read().await.list_sessions().await?;
    let other_sessions: Vec<_> = all_sessions.iter().filter(|s| !s.is_current && s.message_count > 0).collect();

    // Show TODOs and background state if present (from resumed/loaded session)
    let has_todos = session_manager.read().await.todo_manager.has_any_state();
    let has_bg = session_manager.read().await.background_manager.any_tasks();

    if has_todos || has_bg {
        println!();
        println!("{}", "Session state:".dimmed());

        if has_todos {
            let mgr = session_manager.read().await;
            let (pending, in_progress, completed) = mgr.todo_manager.todo_counts();
            let bid = mgr.todo_manager.batch_id().unwrap_or("?");
            println!(
                "  TODOs:         batch {} — {} pending, {} in progress, {} completed",
                bid, pending, in_progress, completed
            );
        }
        if has_bg {
            let m = session_manager.read().await;
            println!(
                "  Background:    {} task(s) ({})",
                m.background_manager.len(),
                if m.background_manager.completed_count() == m.background_manager.len() {
                    "all completed".to_string()
                } else {
                    format!("{} completed, {} incomplete", m.background_manager.completed_count(), m.background_manager.len() - m.background_manager.completed_count())
                }
            );
        }
    }

    println!(
        "{}",
        format!("Session:    {}", current_session).dimmed()
    );

    if !other_sessions.is_empty() {
        println!(
            "{}",
            format!("Use /resume to switch sessions ({} available).", other_sessions.len()).dimmed()
        );
    }

    println!(
        "Mode:      {:?}",
        agent.tools.execution_mode().unwrap_or(ExecutionMode::Default)
    );

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

        if query.starts_with('/') {
            match ui::command::execute_command(&cmd_ctx, &mut agent, &query).await {
                Ok(CmdResult::Handled) => {}
                Ok(CmdResult::Exit) => {
                    if !agent.conversations.is_empty() {
                        if let Err(e) = session_manager.write().await.save_conversation(&agent.conversations).await {
                            eprintln!("Failed to save conversation on exit: {}", e);
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
        if !agent.conversations.is_empty() {
            if let Err(e) = session_manager.read().await.save_conversation(&agent.conversations).await {
                eprintln!("Failed to auto-save conversation: {}", e);
            }
        }
    }

    std::fs::remove_dir_all(&session_manager.read().await.current_session_dir())?;

    // Save history before exit
    if let Err(e) = ui::input::save_history(&mut editor, &data_dir) {
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

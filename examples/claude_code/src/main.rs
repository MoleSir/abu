mod compact;
mod hook;
mod memory;
mod permission;
mod subagent;
mod system_prompt;
mod task;
mod tools;

use std::{
    io::Write,
    sync::Arc,
};

use abu_agent::{
    model::ChatModel,
    tool::ExecutionMode,
    AgentBuilder,
};
use tokio::sync::RwLock;

use compact::{CompactToolResult, SummarizationCompact};
use hook::ClaudeCodeHook;
use memory::{MemoryManager, MemoryMiddleware, MemoryTool};
use permission::build_permission;
use system_prompt::SystemPromptBuilder;
use task::{TaskCreateTool, TaskGetTool, TaskListTool, TaskManager, TaskUpdateTool};
use tools::{init_workdir, Bash, EditFile, ReadFile, WriteFile};

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    }
}

async fn result_main() -> anyhow::Result<()> {
    dotenv::from_filename(".env").ok();

    let workdir = std::env::current_dir()?;
    init_workdir(workdir.clone());

    // Storage directories
    let claude_dir = workdir.join(".claude");
    let memory_dir = claude_dir.join("memory");
    let tasks_dir = claude_dir.join("tasks");
    let tool_results_dir = claude_dir.join("tool_results");

    // Shared managers
    let memory_manager = Arc::new(RwLock::new(MemoryManager::new(&memory_dir)));
    let task_manager = Arc::new(TaskManager::new(&tasks_dir)?);

    // System prompt
    let prompt_builder = SystemPromptBuilder::new(workdir.clone());

    // LLM
    let model = ChatModel::deepseek("deepseek-chat")?;

    // Context compaction
    let compact_model = ChatModel::deepseek("deepseek-chat")?;
    let compact = SummarizationCompact::new(compact_model, 25);
    let tool_result_compactor = CompactToolResult::new(&tool_results_dir)?;

    // Permission
    let permission = build_permission();
    let execution_mode = permission.mode;

    // Subagent
    let subagent = subagent::build_subagent().await?;

    // Build the agent
    let mut agent = AgentBuilder::new(model)
        .system_prompt("")
        .max_iteration(25)
        .with_system_prompt_middleware(prompt_builder)
        .with_system_prompt_middleware(MemoryMiddleware::new(memory_manager.clone()))
        .with_hook(ClaudeCodeHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool(EditFile::new())
        .with_tool(TaskCreateTool::new(task_manager.clone()))
        .with_tool(TaskUpdateTool::new(task_manager.clone()))
        .with_tool(TaskListTool::new(task_manager.clone()))
        .with_tool(TaskGetTool::new(task_manager.clone()))
        .with_tool(MemoryTool::new(memory_manager.clone()))
        .with_subagent(subagent)
        .compact(compact)
        .with_tool_result_middleware(tool_result_compactor)
        .with_permission(permission)
        .build()
        .await?;

    println!("Claude Code (abu-agent)");
    println!("Type /help for commands, q to quit.");

    // REPL loop
    loop {
        print!("> ");
        std::io::stdout().flush()?;

        let mut query = String::new();
        std::io::stdin().read_line(&mut query)?;
        let query = query.trim().to_string();

        if query.is_empty() {
            continue;
        }

        // Handle slash commands
        if query.starts_with('/') {
            handle_command(&query, execution_mode, &memory_manager).await;
            continue;
        }

        // Handle exit
        if query == "q" || query == "quit" {
            println!("Bye!");
            break;
        }

        // Run the agent
        if let Err(e) = agent.run(&query).await {
            eprintln!("Error: {:?}", e);
        }
    }

    Ok(())
}

async fn handle_command(
    cmd: &str,
    execution_mode: ExecutionMode,
    memory_manager: &Arc<RwLock<MemoryManager>>,
) {
    match cmd {
        "/help" => {
            println!("Commands:");
            println!("  /help    - Show this help");
            println!("  /mode    - Show permission execution mode");
            println!("  /memory  - List saved memories");
            println!("  /tasks   - List current tasks");
            println!("  q, quit  - Exit");
        }
        "/mode" => {
            println!("Execution mode: {:?}", execution_mode);
        }
        "/memory" => {
            let mm = memory_manager.read().await;
            let names = mm.list_memories();
            if names.is_empty() {
                println!("No memories saved.");
            } else {
                println!("Saved memories:");
                for name in names {
                    println!("  - {}", name);
                }
            }
        }
        "/tasks" => {
            let task_manager = task::TaskManager::new(".claude/tasks");
            match task_manager {
                Ok(tm) => match tm.list_all() {
                    Ok(list) => println!("{}", list),
                    Err(e) => println!("Error listing tasks: {}", e),
                },
                Err(e) => println!("Error: {}", e),
            }
        }
        _ => {
            println!(
                "Unknown command: {}. Type /help for available commands.",
                cmd
            );
        }
    }
}

use std::{convert::Infallible, io::Write, path::{Path, PathBuf}, process::Stdio, sync::{mpsc, Arc, OnceLock, RwLock}, thread, time::Duration};
use abu_agent::{hook::ConsoleLoggerHook, middleware::{MemoryAddMiddleware, MiddlewareFlow, ToolCallMiddleware}, model::ChatModel, AgentBuilder};
use abu_provider::chat::ToolCall;
use std::process::Command;

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;
    let prompt = "
You are a coding agent.
STRICT RULE: Every new request MUST begin with an `update_todo` call to initialize the plan.
Even for simple tasks, create a plan with at least 2-3 granular steps.
Example steps for refactoring: 1. Read source, 2. Apply changes, 3. Verify.
Keep exactly one step in_progress. Refresh the plan as work advances.
Prefer tools over prose.";

    let model = ChatModel::deepseek("deepseek-chat")?;
    let todo_manager = TodoManager::new();
    let mut agent = AgentBuilder::new(model)
        .max_iteration(20)
        .system_prompt(prompt)
        .with_builtin_tools(false)
        .with_hook(ConsoleLoggerHook::new())
        .with_tool(todo_manager.clone())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool_call_middleware(todo_manager.clone())
        .with_memory_add_middleware(todo_manager.clone())
        .build().await?;
    
    loop {
        print!("s03 >> ");
        std::io::stdout().flush()?;
        
        let mut query = String::new();
        std::io::stdin().read_line(&mut query)?;
        let query = query.trim();
        if query == "q" || query == "quit" || query.is_empty() {
            break;
        }
        
        agent.run(query).await?;
    }

    Ok(())
}

// ====================================================================== //
//                      TODO Writor
// ====================================================================== //

const PLAN_REMINDER_INTERVAL: usize = 5;

#[derive(Clone)]
pub struct TodoManager(Arc<RwLock<TodoManagerImpl>>);

impl TodoManager {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(TodoManagerImpl::new())))
    } 
}

#[abu_tool::tool(
    struct_name = TodoManager,
    description = "Rewrite the current session plan for multi-step work.",
    name = "update_todo"
)]
pub fn update(&self, items: Vec<PlanItem>) -> Result<String, String> {
    self.0.write().unwrap().update(items)
}

#[async_trait::async_trait]
impl ToolCallMiddleware for TodoManager {
    type Error = Infallible;
    async fn intercept(&mut self, tool_call: &mut ToolCall) -> Result<MiddlewareFlow, Self::Error> {
        // 是否调用 update_todo?
        if tool_call.name == "update_todo" {
            self.0.write().unwrap().used_todo = true;
        } else {
            self.0.write().unwrap().used_todo = false;
        }

        Ok(MiddlewareFlow::Continue)
    }
}

#[async_trait::async_trait]
impl MemoryAddMiddleware for TodoManager {
    type Error = Infallible;
    async fn intercept(&mut self, _user_input: &str, ai_response: &mut String) -> Result<MiddlewareFlow, Self::Error> {
        if self.0.read().unwrap().used_todo {
            self.0.write().unwrap().rounds_since_update = 0;
        } else {
            self.0.write().unwrap().note_round_without_update();
            if let Some(reminder) = self.0.read().unwrap().reminder() {
                ai_response.push_str(&reminder);
            }
        }
        Ok(MiddlewareFlow::Continue)
    }
}

#[derive(Default)]
pub struct TodoManagerImpl {
    pub items: Vec<PlanItem>,
    pub rounds_since_update: usize,
    pub used_todo: bool,
}

#[abu_tool::tool_argument]
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Pending,
    InProgress,
    Completed,
}

impl Status {
    pub fn marker(&self) -> &'static str {
        match self {
            Status::Pending => "[ ]",
            Status::InProgress => "[>]",
            Status::Completed => "[x]",
        }
    }
}

#[abu_tool::tool_argument]
#[derive(Debug)]
pub struct PlanItem {
    pub content: String,
    pub status: Status,
}

impl TodoManagerImpl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, items: Vec<PlanItem>) -> Result<String, String> {
        if items.len() > 12 {
            return Err("Keep the session plan short (max 12 items)".to_string());
        }
    
        let mut in_progress_count = 0;
    
        for (index, item) in items.iter().enumerate() {
            if item.content.is_empty() {
                return Err(format!("PlanItem {}: content required", index));
            }
    
            if item.status == Status::InProgress {
                in_progress_count += 1;
            }
        }
    
        if in_progress_count > 1 {
            return Err("Only one plan item can be in_progress".to_string());
        }
    
        self.items = items;
    
        Ok(self.render())
    }

    pub fn note_round_without_update(&mut self) {
        self.rounds_since_update += 1;
    }

    pub fn reminder(&self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }
        if self.rounds_since_update < PLAN_REMINDER_INTERVAL {
            return None;
        }
        Some("<reminder>Refresh your current plan before continuing.</reminder>".to_string())
    }

    pub fn render(&self) -> String {
        if self.items.is_empty() {
            return "No session plan yet.".to_string();
        }

        let mut lines = Vec::new();
        for item in &self.items {
            let line = format!("{} {}", item.status.marker(), item.content);
            lines.push(line);
        }

        let completed = self.items.iter().filter(|i| i.status == Status::Completed).count();        
        lines.push(format!("\n({}/{} completed)", completed, self.items.len()));

        lines.join("\n")
    }
}

// ====================================================================== //
//                      Tool
// ====================================================================== //

static WORKDIR: OnceLock<PathBuf> = OnceLock::new();

fn get_workdir() -> &'static PathBuf {
    WORKDIR.get_or_init(|| std::env::current_dir().expect("Failed to get current working directory"))
}

/// 解析并验证路径，防止目录穿越 (Directory Traversal)
fn safe_path<P: AsRef<Path>>(p: P) -> anyhow::Result<PathBuf> {
    let workdir = get_workdir();
    let mut path = workdir.clone();

    use std::path::Component;
    for component in p.as_ref().components() {
        match component {
            Component::ParentDir => { path.pop(); },
            Component::Normal(c) => { path.push(c); }
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
        }
    }

    if !path.starts_with(workdir) {
        anyhow::bail!("Path escapes workspace: {:?}", p.as_ref())
    }

    Ok(path)
}

/// 运行 Shell 命令并带有 120 秒超时限制
#[abu_macros::tool(
    struct_name = Bash,
    description = "Run a shell command.",
)]
pub fn run_bash(command: &str) -> String {
    // 过滤危险命令
    let dangerous = ["rm -rf /", "sudo", "shutdown", "reboot", "> /dev/"];
    if dangerous.iter().any(|&d| command.contains(d)) {
        return "Error: Dangerous command blocked".to_string();
    }
    
    let (shell, arg) = ("sh", "-c");
    let cmd_str = command.to_string();

    // 创建通道通信
    let (tx, rx) = mpsc::channel();

    // 在新线程中运行命令以实现超时控制
    thread::spawn(move || {
        let output = Command::new(shell)
            .arg(arg)
            .arg(&cmd_str)
            .current_dir(get_workdir())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let _ = tx.send(output);
    }); 

    // 等待结果，超时时间 120 秒
    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(output)) => {
            let mut out = String::from_utf8_lossy(&output.stdout).into_owned();
            let err = String::from_utf8_lossy(&output.stderr);
            out.push_str(&err);
            let out = out.trim();
            
            if out.is_empty() {
                "(no output)".to_string()
            } else {
                // 截断前 50000 个字符
                out.chars().take(50000).collect()
            }
        }
        Ok(Err(e)) => format!("Error: {}", e),
        Err(_) => "Error: Timeout (120s)".to_string(),
    }
}

/// 读取文件
#[abu_macros::tool(
    struct_name = ReadFile,
    description = "Read file contents.",
)]
pub fn run_read(path: &str) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    match std::fs::read_to_string(&fp) {
        Ok(t) => t,
        Err(e) => format!("Error: {}", e),
    }
}

/// 写入文件，自动创建父目录
#[abu_macros::tool(
    struct_name = WriteFile,
    description = "Write content to file.",
)]
pub fn run_write(path: &str, content: &str) -> String {
    let fp = match safe_path(path) {
        Ok(p) => p,
        Err(e) => return format!("Error: {}", e),
    };

    if let Some(parent) = fp.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return format!("Error: {}", e);
        }
    }

    match std::fs::write(&fp, content) {
        Ok(_) => format!("Wrote {} bytes to {}", content.len(), path),
        Err(e) => format!("Error: {}", e),
    }
}
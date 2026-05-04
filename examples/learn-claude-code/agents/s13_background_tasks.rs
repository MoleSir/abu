use std::{collections::HashMap, io::Write, path::{Path, PathBuf}, process::Stdio, sync::{mpsc, Arc, OnceLock}, thread, time::Duration};
use abu_agent::{hook::ConsoleLoggerHook, middleware::{LlmInputMiddleware, MiddlewareFlow}, model::ChatModel, AgentBuilder};
use abu_provider::chat::ChatMessage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::{fs, process::Command, sync::Mutex};
use uuid::Uuid;

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;
    let manager = Arc::new(BackgroundManager::new(".runtime-tasks")?);
    let model = ChatModel::deepseek("deepseek-chat")?;
    let cur_path = std::env::current_dir()?;
    println!("{:?}",cur_path);
    let mut agent = AgentBuilder::new(model)
        .system_prompt(format!("You are a coding agent at {:?}. Use background_run for long-running commands.", cur_path))
        .with_hook(ConsoleLoggerHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool(BackgroundRunTool::new(manager.clone()))
        .with_tool(BackgroundCheckTool::new(manager.clone()))
        .with_tool(BackgroundCheckAllTool::new(manager.clone()))
        .with_llm_input_middleware(BackgroundMiddleware::new(manager.clone()))
        .build().await?;

    loop {
        print!("s13 >> ");
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
//                      Wrap Middleware
// ====================================================================== //

pub struct BackgroundMiddleware {
    pub manager: Arc<BackgroundManager>,
}

impl BackgroundMiddleware {
    pub fn new(manager: Arc<BackgroundManager>,) -> Self {
        Self { manager }
    }
}

#[async_trait::async_trait]
impl LlmInputMiddleware for BackgroundMiddleware {
    type Error = anyhow::Error;
    async fn intercept(&mut self, messages: &mut Vec<ChatMessage>) -> Result<MiddlewareFlow, Self::Error> {
        // 在每次请求 llm 之前，插入后台任务结果
        let notifs = self.manager.drain_notifications().await;
        if !notifs.is_empty() {
            let notifs: Vec<_> = notifs.iter().map(|notif| {
                format!("[bg:{}] {}: {} (output_file={})", notif.task_id, notif.status.to_str(), notif.preview, notif.output_file)
            }).collect();
            let notif_text = notifs.join("\n");
            messages.push(ChatMessage::user(format!("<background-results>\n{notif_text}\n</background-results>")));
        }
        Ok(MiddlewareFlow::Continue)
    }
}

// ====================================================================== //
//                      Wrap Tool
// ====================================================================== //

pub struct BackgroundRunTool {
    pub manager: Arc<BackgroundManager>,
}

impl BackgroundRunTool {
    pub fn new(manager: Arc<BackgroundManager>,) -> Self {
        Self { manager }
    }
}

#[abu_tool::tool(
    struct_name = BackgroundRunTool,
    name = "run_background",
    description = "Run command in background thread. Returns task_id immediately."
)] 
pub async fn save_memory(&self, command: String) -> anyhow::Result<String> {
    self.manager.run(command).await
}

pub struct BackgroundCheckTool {
    pub manager: Arc<BackgroundManager>,
}

impl BackgroundCheckTool {
    pub fn new(manager: Arc<BackgroundManager>,) -> Self {
        Self { manager }
    }
}

#[abu_tool::tool(
    struct_name = BackgroundCheckTool,
    name = "check_background",
    description = "Check background task status."
)] 
pub async fn save_memory(&self, task_id: &str) -> String {
    self.manager.check(task_id).await
}

pub struct BackgroundCheckAllTool {
    pub manager: Arc<BackgroundManager>,
}

impl BackgroundCheckAllTool {
    pub fn new(manager: Arc<BackgroundManager>,) -> Self {
        Self { manager }
    }
}

#[abu_tool::tool(
    struct_name = BackgroundCheckAllTool,
    name = "check_all_background",
    description = "List all background tasks status."
)] 
pub async fn save_memory(&self) -> String {
    self.manager.check_all().await
}

// ====================================================================== //
//                      Background
// ====================================================================== //

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TaskStatus {
    Running,
    Completed,
    Timeout,
    Error,
}

impl TaskStatus {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Timeout => "timeout",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub id: String,
    pub status: TaskStatus,
    pub command: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub result_preview: String,
    pub output_file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Notification {
    pub task_id: String,
    pub status: TaskStatus,
    pub command: String,
    pub preview: String,
    pub output_file: String,
}

#[derive(Default)]
struct ManagerState {
    tasks: HashMap<String, Task>,
    notifications: Vec<Notification>,
}

pub struct BackgroundManager {
    runtime_dir: PathBuf,
    state: Arc<Mutex<ManagerState>>,
}

/// 后台任务管理
/// - 调用 run 函数创建一个异步任务，将任务信息插入 state 中，同时创建 json 文件记录
/// - 当异步任何完成，将结果写入 log 文件 / 更新 json 文件记录，将完成的通知插入 state
impl BackgroundManager {
    pub fn new<P: Into<PathBuf>>(runtime_dir: P) -> anyhow::Result<Self> {
        let runtime_dir = runtime_dir.into();
        if !runtime_dir.exists() {
            std::fs::create_dir_all(&runtime_dir)?;
        }

        Ok(Self {
            runtime_dir,
            state: Arc::new(Mutex::new(ManagerState::default()))
        })
    }

    pub async fn run(&self, command: String) -> anyhow::Result<String> {
        let task_id = Uuid::new_v4().to_string()[..8].to_string();
        let output_file = self.output_path(&task_id);

        // 任务信息
        let task = Task {
            id: task_id.clone(),
            status: TaskStatus::Running,
            command: command.clone(),
            started_at: Utc::now(),
            finished_at: None,
            result_preview: String::new(),
            output_file: output_file.to_string_lossy().into_owned(),
        };

        // 写入文件 / 保存信息
        self.persist_task(&task).await?;
        {
            let mut state = self.state.lock().await;
            state.tasks.insert(task_id.clone(), task);
        }

        // 开启异步任务
        let state_clone = self.state.clone();
        let runtime_dir_clone = self.runtime_dir.clone();
        let task_id_clone = task_id.clone();

        tokio::spawn(async move {
            // 执行并解析返回结果
            let result = Self::execute_internal(&command).await;
            let (status, output) = match result {
                Ok(out) => (TaskStatus::Completed, out),
                Err(e) if e.to_string().contains("timed out") => (TaskStatus::Timeout, format!("Error: {}", e)),
                Err(e) => (TaskStatus::Error, format!("Error: {}", e)),
            };

            let preview = Self::create_preview(&output);
            let finished_at = Utc::now();

            let mut state_guard = state_clone.lock().await;
    
            let ManagerState { tasks, notifications } = &mut *state_guard;
            if let Some(t) = tasks.get_mut(&task_id_clone) {
                // 更新任务状态
                t.status = status.clone();
                t.finished_at = Some(finished_at);
                t.result_preview = preview.clone();
                
                // 写日志文件
                let _ = fs::write(runtime_dir_clone.join(format!("{}.log", task_id_clone)), &output).await;
                                
                // 写 JSON 记录
                let content = serde_json::to_string_pretty(&t).unwrap_or_default();
                let _ = fs::write(runtime_dir_clone.join(format!("{}.json", task_id_clone)), content).await;

                // 【现在可以了】：notifications 和 tasks 是独立的借用
                notifications.push(Notification {
                    task_id: task_id_clone,
                    status,
                    command: t.command.chars().take(80).collect(),
                    preview,
                    output_file: t.output_file.clone(),
                });
            }
        });

        Ok(format!("Background task {} started.", task_id))
    }

    /// 输出指定 task 的状态
    pub async fn check(&self, task_id: &str) -> String {
        let state = self.state.lock().await;
        state.tasks.get(task_id)
            .map(|t| serde_json::to_string_pretty(t).unwrap_or_default())
            .unwrap_or_else(|| format!("Error: Unknown task {}", task_id))
    }

    /// 获取所有任务状态
    pub async fn check_all(&self) -> String {
        let state = self.state.lock().await;
        if state.tasks.is_empty() {
            "No background tasks.".to_string()
        } else {
            state.tasks.values()
                .map(|t| format!("{}: [{:?}] {} -> {}", 
                    t.id, t.status, 
                    &t.command[..t.command.len().min(60)], 
                    t.result_preview))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    pub async fn drain_notifications(&self) -> Vec<Notification> {
        let mut state = self.state.lock().await;
        std::mem::take(&mut state.notifications)
    }

    pub async fn detect_stalled(&self, threshold_secs: u64) -> Vec<String> {
        let state = self.state.lock().await;
        let now = Utc::now();
        state.tasks.values()
            .filter(|t| matches!(t.status, TaskStatus::Running))
            .filter(|t| (now - t.started_at).num_seconds() > threshold_secs as i64)
            .map(|t| t.id.clone())
            .collect()
    }

    async fn execute_internal(command: &str) -> anyhow::Result<String> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(command)
            .output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
    
        let mut final_output = format!("{}{}", stdout, stderr);
        if final_output.is_empty() {
            final_output = "(no output)".to_string();
        }
        
        Ok(final_output.chars().take(50000).collect())
    }

    /// 将任务写入文件
    async fn persist_task(&self, task: &Task) -> anyhow::Result<()> {
        let contents = serde_json::to_string_pretty(task)?;
        fs::write(self.record_path(&task.id), contents).await?;
        Ok(())
    }

    fn create_preview(output: &str) -> String {
        let compact: String = output
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if compact.len() > 500 {
            format!("{}...", &compact[..497])
        } else {
            compact
        }
    }

    /// task 的信息路径
    fn record_path(&self, task_id: &str) -> PathBuf {
        self.runtime_dir.join(format!("{}.json", task_id))
    }

    /// task 的输出路径
    fn output_path(&self, task_id: &str) -> PathBuf {
        self.runtime_dir.join(format!("{}.log", task_id))
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
    let p = p.as_ref();

    let path = if p.is_absolute() {
        p.to_path_buf()
    } else {
        workdir.join(p)
    };

    let path = path.canonicalize()?; // 解析符号链接等

    if !path.starts_with(workdir) {
        anyhow::bail!("Path escapes workspace: {:?}", p);
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
        let output = std::process::Command::new(shell)
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
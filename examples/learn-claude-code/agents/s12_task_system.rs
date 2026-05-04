use std::{io::Write, path::{Path, PathBuf}, process::Stdio, sync::{mpsc, Arc, OnceLock}, thread, time::Duration};
use abu_agent::{hook::ConsoleLoggerHook, model::ChatModel, AgentBuilder};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;
    let task_manager = Arc::new(TaskManager::new(".task_dir")?);
    
    let model = ChatModel::deepseek("deepseek-chat")?;
    let cur_path = std::env::current_dir()?;
    println!("{:?}",cur_path);
    let mut agent = AgentBuilder::new(model)
        .system_prompt(format!("You are a coding agent at {:?}.", cur_path))
        .with_hook(ConsoleLoggerHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool(TaskCreateTool::new(task_manager.clone()))
        .with_tool(TaskListTool::new(task_manager.clone()))
        .with_tool(TaskUpdateTool::new(task_manager.clone()))
        .with_tool(TaskGetTool::new(task_manager.clone()))
        .build().await?;

    loop {
        print!("s12 >> ");
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
//                      Wrap Tool
// ====================================================================== //

pub struct TaskCreateTool(Arc<TaskManager>);
impl TaskCreateTool {
    pub fn new(t: Arc<TaskManager>) -> Self { Self(t) }
}

#[abu_tool::tool(
    struct_name = TaskCreateTool,
    name = "task_create",
    description = "Create a new task."
)] 
pub async fn task_create(&self, subject: &str, description: &str) -> anyhow::Result<String> {
    self.0.create(subject, description)
}

pub struct TaskUpdateTool(Arc<TaskManager>);
impl TaskUpdateTool {
    pub fn new(t: Arc<TaskManager>) -> Self { Self(t) }
}

#[abu_tool::tool(
    struct_name = TaskUpdateTool,
    name = "task_update",
    description = "Update a task's status"
)] 
pub async fn task_update(&self, task_id: u32, status: TaskStatus) -> anyhow::Result<String> {
    self.0.update(task_id, status)
}

pub struct TaskListTool(Arc<TaskManager>);
impl TaskListTool {
    pub fn new(t: Arc<TaskManager>) -> Self { Self(t) }
}

#[abu_tool::tool(
    struct_name = TaskListTool,
    name = "task_list",
    description = "List all tasks with status summary."
)] 
pub async fn task_list(&self) -> anyhow::Result<String> {
    self.0.list_all()
}

pub struct TaskGetTool(Arc<TaskManager>);
impl TaskGetTool {
    pub fn new(t: Arc<TaskManager>) -> Self { Self(t) }
}

#[abu_tool::tool(
    struct_name = TaskGetTool,
    name = "task_get",
    description = "Get full details of a task by ID."
)] 
pub async fn task_get(&self, task_id: u32) -> anyhow::Result<String> {
    self.0.get(task_id)
}

// ====================================================================== //
//                      TaskManager
// ====================================================================== //

#[abu_tool::tool_argument]
#[derive(Serialize)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Deleted,
}

#[derive(Serialize, Deserialize)]
pub struct Task {
    pub id: u32,
    pub subject: String,
    pub description: String,
    pub status: TaskStatus,
}

pub struct TaskManager {
    pub tasks_dir: PathBuf,
}

impl TaskManager {
    pub fn new<P: Into<PathBuf>>(tasks_dir: P) -> anyhow::Result<Self> {
        let tasks_dir = tasks_dir.into();
        std::fs::create_dir_all(&tasks_dir)?;
        Ok(Self { tasks_dir })
    }

    pub fn create(&self, subject: &str, description: &str) -> anyhow::Result<String> {
        let task_id = self.next_id()?;
        let task = Task {
            id: task_id,
            subject: subject.to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
        };
        self.save(&task)
    }

    pub fn update(&self, task_id: u32, status: TaskStatus) -> anyhow::Result<String> {
        let mut task = self.load(task_id)?;
        task.status = status;
        self.save(&task)
    }

    pub fn get(&self, task_id: u32) -> anyhow::Result<String> {
        let task = self.load(task_id)?;
        let content = serde_json::to_string_pretty(&task)?;
        Ok(content)
    }

    pub fn list_all(&self) -> anyhow::Result<String> {
        let mut tasks: Vec<Task> = vec![];

        for entry in std::fs::read_dir(&self.tasks_dir)?.flatten() {
            let path = entry.path();

            let file_name = path.file_name().unwrap().to_str().unwrap();
            if file_name.starts_with("task_") && file_name.ends_with(".json") {
                let content = std::fs::read_to_string(&path)?;
                tasks.push(serde_json::from_str(&content)?);
            }
        }

        if tasks.is_empty() {
            Ok("No tasks".to_string())
        } else {
            let mut lines = vec![];
            for task in tasks {
                let markder = match task.status {
                    TaskStatus::Pending => "[ ]",
                    TaskStatus::InProgress => "[>]",
                    TaskStatus::Completed => "[x]",
                    TaskStatus::Deleted => "[-]",
                };
                lines.push(format!("{} #{}: {}", markder, task.id, task.subject));
            }
            Ok(lines.join("\n"))
        }
    }

    fn save(&self, task: &Task) -> anyhow::Result<String> {
        let path = self.tasks_dir.join(format!("task_{}.json", task.id));
        let content = serde_json::to_string_pretty(task)?;
        std::fs::write(path, &content)?;
        Ok(content)
    }
 
    fn load(&self, task_id: u32) -> anyhow::Result<Task> {
        let path = self.tasks_dir.join(format!("task_{}.json", task_id));
        let content = std::fs::read_to_string(&path)?;
        let task = serde_json::from_str(&content)?;
        Ok(task)
    }

    fn next_id(&self) -> anyhow::Result<u32> {
        let mut max_id = 0;

        for entry in std::fs::read_dir(&self.tasks_dir)?.flatten() {
            let path = entry.path();

            let file_name = path.file_name().unwrap().to_str().unwrap();
            if file_name.starts_with("task_") && file_name.ends_with(".json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(id_part) = stem.split('_').nth(1) {
                        if let Ok(id) = id_part.parse::<u32>() {
                            max_id = max_id.max(id);
                        }
                    }
                }
            }
        }

        Ok(max_id)
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
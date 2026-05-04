use std::{hash::{DefaultHasher, Hash, Hasher}, io::Write, path::{Path, PathBuf}, process::Stdio, sync::{atomic::{AtomicBool, Ordering}, Arc, OnceLock}, thread, time::Duration};
use abu_agent::{hook::ConsoleLoggerHook, middleware::{LlmInputMiddleware, MiddlewareFlow}, model::ChatModel, AgentBuilder};
use abu_provider::chat::ChatMessage;
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, task::JoinHandle};
use uuid::Uuid;
use std::process::Command;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;

    let cron_scheduler = CronScheduler::new("./")?;
    let cron_scheduler = Arc::new(Mutex::new(cron_scheduler));
    cron_scheduler.lock().await.start().await?;
    
    let res = agent_main(cron_scheduler.clone()).await;
    cron_scheduler.lock().await.stop().await;
    res
}

async fn agent_main(cron_scheduler: Arc<Mutex<CronScheduler>>) -> anyhow::Result<()> {
    let model = ChatModel::deepseek("deepseek-chat")?;
    let cur_path = std::env::current_dir()?;
    println!("{:?}",cur_path);
    let mut agent = AgentBuilder::new(model)
        .system_prompt(format!("You are a coding agent at {:?}. Use tools to solve tasks.\n\nYou can schedule future work with cron_create. Tasks fire automatically and their prompts are injected into the conversation.", cur_path))
        .with_hook(ConsoleLoggerHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool(CronCreateTool::new(cron_scheduler.clone()))
        .with_tool(CronDeleteTool::new(cron_scheduler.clone()))
        .with_tool(CronListTool::new(cron_scheduler.clone()))
        .with_llm_input_middleware(CronMiddleware::new(cron_scheduler.clone()))
        .build().await?;

    loop {
        print!("s14 >> ");
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
//                      CronLock
// ====================================================================== //

const CRON_LOCK_FILE: &'static str = "cron.lock";

/// PID-file-based lock to prevent multiple sessions from firing the same cron job.
pub struct CronLock {
    lock_path: PathBuf
}

impl CronLock {
    pub fn new() -> Self {
        let lock_path = get_workdir().join(CRON_LOCK_FILE);
        Self { lock_path }
    }

    /// Try to acquire the cron lock. Returns True on success.
    /// If a lock file exists, check whether the PID inside is still alive.
    /// If the process is dead the lock is stale and we can take over.
    pub fn acquire(&self) -> anyhow::Result<bool> {
        if self.lock_path.exists() {
            let stored_pid = self.read_pid()?;
            if Self::is_process_alive(stored_pid) {
                return Ok(false);
            }
        }

        // 写入当前进程的 PID
        let current_pid = std::process::id();
        std::fs::write(&self.lock_path, current_pid.to_string())?;
        Ok(false)
    }

    /// Remove the lock file if it belongs to this process.
    pub fn release(&self) -> anyhow::Result<()> {
        if self.lock_path.exists() {
            if self.lock_path.exists() {
                let stored_pid = self.read_pid()?;
                if stored_pid == std::process::id() as i32 {
                    std::fs::remove_file(&self.lock_path)?;
                }
            }
        }
        Ok(())
    }

    fn read_pid(&self) -> anyhow::Result<i32> {
        let content = std::fs::read_to_string(&self.lock_path)?;
        let stored_pid = content.trim().parse::<i32>()?;
        Ok(stored_pid)
    }

    #[cfg(unix)]
    fn is_process_alive(pid: i32) -> bool {
        unsafe { libc::kill(pid, 0) == 0 }
    }

    #[cfg(not(unix))]
    fn is_process_alive(_pid: i32) -> bool {
        unimplemented!("Process liveness check using kill(pid, 0) is only supported on Unix systems.")
    }
}

// ====================================================================== //
//                      Warp Middleware
// ====================================================================== //

pub struct CronMiddleware(Arc<Mutex<CronScheduler>>);
impl CronMiddleware {
    pub fn new(inner: Arc<Mutex<CronScheduler>>) -> Self { Self(inner) }
}

#[async_trait::async_trait]
impl LlmInputMiddleware for CronMiddleware {
    type Error = anyhow::Error;
    async fn intercept(&mut self, messages: &mut Vec<ChatMessage>) -> Result<MiddlewareFlow, Self::Error> {
        let iter = self.0.lock().await
            .drain_notifications().into_iter()
            .map(|notification| {
                ChatMessage::user(format!("[Cron notification] {notification}"))
            });
        messages.extend(iter);
        Ok(MiddlewareFlow::Continue)
    }
}

// ====================================================================== //
//                      Warp Tool
// ====================================================================== //

pub struct CronCreateTool(Arc<Mutex<CronScheduler>>);
impl CronCreateTool {
    pub fn new(inner: Arc<Mutex<CronScheduler>>) -> Self { Self(inner) }
}

#[abu_tool::tool(
    struct_name = CronCreateTool,
    name = "cron_create",
    description = "Schedule a recurring or one-shot task with a cron expression."
)] 
pub async fn cron_create(&self,
    #[arg(description = "5-field cron expression: 'min hour dom month dow'")]
    cron_expr: String,
    #[arg(description = "The prompt to inject when the task fires")]
    prompt: String,
    #[arg(description = "true=repeat, false=fire once then delete. Default true.")]
    recurring: bool,
    #[arg(description = "true=persist to disk, false=session-only. Default false.")]
    durable: bool,
) -> anyhow::Result<String> {
    self.0.lock().await.create(cron_expr, prompt, recurring, durable).await
}

pub struct CronDeleteTool(Arc<Mutex<CronScheduler>>);
impl CronDeleteTool {
    pub fn new(inner: Arc<Mutex<CronScheduler>>) -> Self { Self(inner) }
}

#[abu_tool::tool(
    struct_name = CronDeleteTool,
    name = "cron_delete",
    description = "Schedule a recurring or one-shot task with a cron expression."
)] 
pub async fn cron_delete(
    &self,
    #[arg(description = "Task ID to delete")]
    task_id: &str,
) -> anyhow::Result<String> {
    self.0.lock().await.delete(task_id).await
}

pub struct CronListTool(Arc<Mutex<CronScheduler>>);
impl CronListTool {
    pub fn new(inner: Arc<Mutex<CronScheduler>>) -> Self { Self(inner) }
}

#[abu_tool::tool(
    struct_name = CronListTool,
    name = "cron_list",
    description = "Schedule a recurring or one-shot task with a cron expression."
)] 
pub async fn cron_list(&self) -> anyhow::Result<String> {
    self.0.lock().await.list_tasks().await
}

// ====================================================================== //
//                      CronScheduler
// ====================================================================== //

#[derive(Clone, Deserialize, Serialize)]
pub struct Task {
    pub id: String,
    pub cron_expr: String,
    pub prompt: String,
    pub recurring: bool,
    pub durable: bool,
    pub created_at: DateTime<Utc>,
    pub jitter_offset: Option<u64>,
    pub last_fired: Option<DateTime<Utc>>, 
}

pub struct CronScheduler {
    pub tasks: Arc<Mutex<Vec<Task>>>,
    pub scheduler_task_file: PathBuf,
    /// 用来接受后台 thread 发送的任务
    notification_rx: Option<mpsc::Receiver<String>>,
    /// 控制后台任务是否停止
    stop_flag: Arc<AtomicBool>,
    /// 保存后台任务对象
    worker_thread: Option<JoinHandle<()>>,
}

impl CronScheduler {
    pub fn new<P: Into<PathBuf>>(work_dir: P) -> anyhow::Result<Self> {
        let work_dir: PathBuf = work_dir.into();
        let scheduler_task_file = work_dir.join("scheduled_tasks.json");
        Ok(Self {
            tasks: Arc::new(Mutex::new(vec![])),
            scheduler_task_file,
            notification_rx: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
            worker_thread: None,
        })
    }

    /// Load durable tasks and start the background check thread.
    pub async fn start(&mut self) -> anyhow::Result<()> {
        // 加载文件中的任务
        self.load_durable().await?;

        // 创建无界通道
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(5000);
        // 保存接受通道
        self.notification_rx = Some(rx);
        self.stop_flag.store(false, Ordering::SeqCst);

        // 克隆需要在后台线程中使用的变量的引用 (Arc)
        let tasks_arc = Arc::clone(&self.tasks);
        let stop_flag_arc = Arc::clone(&self.stop_flag);
        let file_path = self.scheduler_task_file.clone();

        // 启动后台守护线程
        let handle = tokio::spawn(async move {
            let mut last_check_minute = -1;

            while !stop_flag_arc.load(Ordering::Relaxed) {
                let now = Utc::now();
                let current_minute = (now.hour() * 60 + now.minute()) as i32;

                // 确保每分钟只检查一次，防止重复触发
                if current_minute != last_check_minute {
                    last_check_minute = current_minute;
                    
                    // 执行具体的检查逻辑
                    Self::check_tasks(&tasks_arc, &file_path, &tx, now).await;
                }

                // 类似 Python 的 sleep / wait
                thread::sleep(Duration::from_secs(1));
            }
        });

        self.worker_thread = Some(handle);
        
        let count = self.tasks.lock().await.len();
        if count > 0 {
            println!("[Cron] Loaded {} scheduled tasks", count);
        }

        Ok(())
    }

    /// 停止后台线程
    pub async fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker_thread.take() {
            let _ = handle.await;
        }
    }

    /// 提取所有队列中的通知
    pub fn drain_notifications(&mut self) -> Vec<String> {
        let mut notifications = vec![];
        if let Some(rx) = &mut self.notification_rx {
            // try_recv 不会阻塞，拿空了就会返回 Err(TryRecvError::Empty)
            while let Ok(msg) = rx.try_recv() {
                notifications.push(msg);
            }
        }
        notifications
    }

    /// 后台线程专用逻辑，检查并触发任务
    async fn check_tasks(
        tasks_arc: &Arc<Mutex<Vec<Task>>>, 
        file_path: &PathBuf,
        tx: &mpsc::Sender<String>,
        now: DateTime<Utc>
    ) {
        let mut tasks = tasks_arc.lock().await;
        let mut should_save = false;

        // 使用 retain 方法原地过滤任务。返回 false 的任务会被从 Vec 中删除。
        tasks.retain_mut(|task| {
            // 1. 自动过期清理: 创建超过 7 天的周期任务自动删除
            let age_days = (now - task.created_at).num_days();
            if task.recurring && age_days > 7 {
                println!("[Cron] Auto-expired: {} (older than 7 days)", task.id);
                should_save = true;
                return false; // 删除此任务
            }

            // 2. 计算 Jitter 偏移量
            let mut check_time = now;
            if let Some(jitter) = task.jitter_offset {
                check_time -= chrono::Duration::minutes(jitter as i64);
            }

            // 3. 匹配时间 (假设你有 cron_matches 函数)
            if cron_matches(&task.cron_expr, &check_time) {
                let notification = format!("[Scheduled task {}]: {}", task.id, task.prompt);
                
                // 将通知推送到通道
                let _ = tx.send(notification);
                
                task.last_fired = Some(Utc::now());
                println!("[Cron] Fired: {}", task.id);

                // 如果是单次任务，触发后删除
                if !task.recurring {
                    println!("[Cron] One-shot completed and removed: {}", task.id);
                    should_save = true;
                    return false; // 删除此任务
                } else {
                    // 周期任务触发后状态改变（last_fired 变化），最好持久化
                    should_save = true; 
                }
            }

            true // 保留此任务
        });

        // 如果有任务被删除或更新，且有需要持久化的任务，写入磁盘
        if should_save {
            let durables: Vec<&Task> = tasks.iter().filter(|t| t.durable).collect();
            if let Ok(contents) = serde_json::to_string_pretty(&durables) {
                let _ = std::fs::write(file_path, contents);
            }
        }
    }

    pub async fn create(&mut self, cron_expr: String, prompt: String, recurring: bool, durable: bool) -> anyhow::Result<String> {
        let mut task = Task {
            id: Uuid::new_v4().to_string(),
            cron_expr: cron_expr.clone(),
            prompt,
            recurring,
            durable,
            created_at: Utc::now(),
            jitter_offset: None,
            last_fired: None,
        };
        
        if recurring {
            task.jitter_offset = Some(self.compute_jitter(&task.cron_expr)?);
        }

        let response = format!(
            "Created task {} ({}, {}): cron={}", 
            task.id, 
            if recurring { "recurring" } else { "one-shot" }, 
            if durable { "durable" } else { "session-only" }, 
            task.cron_expr
        );

        // 获取锁后再 push
        self.tasks.lock().await.push(task);

        if durable {
            self.save_durable().await?;
        } 
        
        Ok(response)
    }

    pub async fn delete(&mut self, task_id: &str) -> anyhow::Result<String> {
        let mut tasks = self.tasks.lock().await;
        let before_size = tasks.len();
        tasks.retain(|t| t.id != task_id);

        if tasks.len() < before_size {
            // 放开锁再去调用 save_durable 防止死锁 (或者把 save_durable 改为内部调用)
            drop(tasks);
            self.save_durable().await?;
            Ok(format!("Deleted task {task_id}"))
        } else {
            Ok(format!("Task {task_id} not found"))
        }
    }

    pub async fn list_tasks(&self) -> anyhow::Result<String> {
        let tasks = self.tasks.lock().await;
        if tasks.is_empty() {
            Ok("No scheduled tasks.".to_string())
        } else {
            let mut lines: Vec<String> = vec![];
            for task in tasks.iter() {
                let mode = if task.recurring { "recurring" } else { "one-shot" };
                let store = if task.durable { "durable" } else { "session-only" };   
                lines.push(format!("  {}  {}  [{}/{}] ", task.id, task.cron_expr, mode, store));
            }
            Ok(lines.join("\n"))
        }
    }

    async fn save_durable(&self) -> anyhow::Result<()> {
        let tasks = self.tasks.lock().await;
        let durables: Vec<&Task> = tasks.iter().filter(|t| t.durable).collect();
        let contents = serde_json::to_string_pretty(&durables)?;
        std::fs::write(&self.scheduler_task_file, contents)?;
        Ok(())
    }

    async fn load_durable(&mut self) -> anyhow::Result<()> {
        if self.scheduler_task_file.exists() {
            let contents = std::fs::read_to_string(&self.scheduler_task_file)?;
            let loaded_tasks: Vec<Task> = serde_json::from_str(&contents)?;
            let mut tasks = self.tasks.lock().await;
            *tasks = loaded_tasks.into_iter().filter(|task| task.durable).collect();
        }
        Ok(())
    }
    
    /// If cron targets :00 or :30, return a small offset (1-4 minutes).
    fn compute_jitter(&self, cron_expr: &str) -> anyhow::Result<u64> {
        let fields: Vec<&str> = cron_expr.split_whitespace().collect();
        if fields.len() < 1 {
            return Ok(0);
        }

        let minute_field = fields[0];
        if let Ok(minute_val) = minute_field.parse::<u64>() {
            if minute_val == 0 || minute_val == 30 {
                return Ok(hash_str(cron_expr) % 4 + 1)
            }
        } 

        Ok(0)
    }
}

fn hash_str(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// 判断一个 5 段式 Cron 表达式是否匹配给定的时间
/// 支持的格式: * (任意), */N (步长), N (精确), N-M (范围), N,M (列表)
pub fn cron_matches<T: TimeZone>(expr: &str, dt: &DateTime<T>) -> bool {
    // 按空白字符分割，必须正好是 5 段
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        return false;
    }
    
    // 提取时间组件并转为 i32 方便后续数学计算
    let minute = dt.minute() as i32;
    let hour = dt.hour() as i32;
    let day = dt.day() as i32;
    let month = dt.month() as i32;
    let cron_dow = dt.weekday().num_days_from_sunday() as i32;

    let values = [minute, hour, day, month, cron_dow];
    let ranges = [(0, 59), (0, 23), (1, 31), (1, 12), (0, 6)];

    // 逐一校验 5 个字段
    for i in 0..5 {
        if !field_matches(fields[i], values[i], ranges[i].0) {
            return false;
        }
    }

    true
}

/// 匹配单个 Cron 字段
fn field_matches(field: &str, value: i32, lo: i32) -> bool {
    if field == "*" {
        return true;
    }

    // 处理逗号分隔的列表，例如 "1,3,5" 或 "0,15,30,45"
    for part in field.split(',') {
        let mut part_str = part;
        let mut step = 1;

        // 处理步长 (Step)，例如 "*/5" 或 "1-10/2"
        if let Some((p, s)) = part_str.split_once('/') {
            part_str = p;
            if let Ok(parsed_step) = s.parse::<i32>() {
                step = parsed_step;
            } else {
                continue; // 步长解析失败，跳过该部分
            }
        }

        if part_str == "*" {
            // 格式: */N
            if (value - lo) % step == 0 {
                return true;
            }
        } else if let Some((start_str, end_str)) = part_str.split_once('-') {
            // 格式: N-M 或 N-M/S
            if let (Ok(start), Ok(end)) = (start_str.parse::<i32>(), end_str.parse::<i32>()) {
                if value >= start && value <= end && (value - start) % step == 0 {
                    return true;
                }
            }
        } else {
            // 格式: N (精确值)
            if let Ok(exact) = part_str.parse::<i32>() {
                if exact == value {
                    return true;
                }
            }
        }
    }

    false
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
    let (tx, rx) = std::sync::mpsc::channel();

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

// ====================================================================== //
//                      Test
// ====================================================================== //

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_cron_matches() {
        // 创建一个测试时间: 2023-10-23 14:30:00 (周一)
        let dt = Utc.with_ymd_and_hms(2023, 10, 23, 14, 30, 0).unwrap();

        // 测试: 每天 14:30
        assert!(cron_matches("30 14 * * *", &dt));
        
        // 测试: 每 5 分钟 (30 是 5 的倍数，(30-0)%5 == 0)
        assert!(cron_matches("*/5 * * * *", &dt));
        
        // 测试: 每 7 分钟 (30 不是 7 的倍数)
        assert!(!cron_matches("*/7 * * * *", &dt));
        
        // 测试: 周一的 14:30 (23号是周一，num_days_from_sunday = 1)
        assert!(cron_matches("30 14 * * 1", &dt));
        
        // 测试: 范围与步长 (分钟 0-40，步长 15。30 在范围内且步长匹配)
        assert!(cron_matches("0-40/15 14 * * *", &dt));
        
        // 测试: 列表 (分钟属于 0, 15, 30, 45)
        assert!(cron_matches("0,15,30,45 * * * *", &dt));
        
        // 测试: 错误匹配
        assert!(!cron_matches("31 14 * * *", &dt)); // 分钟不对
        assert!(!cron_matches("30 15 * * *", &dt)); // 小时不对
    }
}
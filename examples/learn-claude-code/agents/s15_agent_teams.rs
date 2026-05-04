use std::{collections::HashMap, fs::OpenOptions, io::Write, path::{Path, PathBuf}, process::Stdio, sync::{mpsc, Arc, OnceLock}, thread, time::Duration};
use abu_agent::{compact::NoContextCompact, hook::ConsoleLoggerHook, memory::NoMemory, middleware::{LlmInputMiddleware, MiddlewareFlow}, model::ChatModel, Agent, AgentBuilder};
use abu_provider::{chat::ChatMessage, deepseek::DeepSeek};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, task::JoinHandle};
use std::process::Command;

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;
    let message_bus = Arc::new(MessageBus::new(".team")?);
    let team_manager = Arc::new(RwLock::new(TeammateManager::new(message_bus.clone())?));

    let model = ChatModel::deepseek("deepseek-chat")?;
    let cur_path = std::env::current_dir()?;
    let mut agent = AgentBuilder::new(model)
        .max_iteration(30)
        .system_prompt(format!("You are a coding agent at {:?}. Spawn teammates and communicate via inboxes.", cur_path))
        .with_hook(ConsoleLoggerHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool(SendMessageTool::new("lead", message_bus.clone()))
        .with_tool(ReadInboxTool::new("lead", message_bus.clone()))
        .with_tool(ListTeammatesTool::new(team_manager.clone()))
        .with_tool(SpawnTeammateTool::new(team_manager.clone()))
        .with_tool(BroadcastTool::new("lead", team_manager.clone(), message_bus.clone()))
        .with_llm_input_middleware(ReadInboxMiddleware::new("lead", message_bus.clone()))
        .build().await?;

    loop {
        print!("s015 >> ");
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

pub struct ReadInboxMiddleware {
    pub name: String,
    pub bus: Arc<MessageBus>,
}

impl ReadInboxMiddleware {
    pub fn new(name: impl Into<String>, bus: Arc<MessageBus>,) -> Self {
        Self { name: name.into(), bus }
    }
}

#[async_trait::async_trait]
impl LlmInputMiddleware for ReadInboxMiddleware {
    type Error = anyhow::Error;
    async fn intercept(&mut self, messages: &mut Vec<ChatMessage>) -> Result<MiddlewareFlow, Self::Error> {
        // 在每次请求 llm 之前，读取 inbox
        let msgs = self.bus.read_inbox(&self.name)?;
        for msg in msgs {
            let content = serde_json::to_string(&msg)?;
            messages.push(ChatMessage::user(content));
        }
        Ok(MiddlewareFlow::Continue)
    }
}

// ====================================================================== //
//                      Wrap Tool
// ====================================================================== //

pub struct BroadcastTool {
    pub name: String,
    pub bus: Arc<MessageBus>,
    pub team_manager: Arc<RwLock<TeammateManager>>,
}

impl BroadcastTool {
    pub fn new(name: impl Into<String>, team_manager: Arc<RwLock<TeammateManager>>, bus: Arc<MessageBus>) -> Self {
        Self { name: name.into(), team_manager, bus }
    }
}

#[abu_tool::tool(
    struct_name = BroadcastTool,
    name = "broadcast",
    description = "Send a message to all teammates."
)] 
pub async fn broadcast(&self, content: &str) -> anyhow::Result<String> {
    let member_names = self.team_manager.read().await.member_names().await;
    self.bus.broadcast(&self.name, &member_names, content)
}

pub struct SpawnTeammateTool {
    pub team_manager: Arc<RwLock<TeammateManager>>,
}

impl SpawnTeammateTool {
    pub fn new(team_manager: Arc<RwLock<TeammateManager>>,) -> Self {
        Self { team_manager }
    }
}

#[abu_tool::tool(
    struct_name = SpawnTeammateTool,
    name = "spawn_teammate",
    description = "Spawn a persistent teammate that runs in its own thread."
)] 
pub async fn spawn_teammate(&self, name: &str, role: &str, prompt: &str) -> anyhow::Result<String> {
    self.team_manager.write().await.spawn(name, role, prompt).await
}

pub struct ListTeammatesTool {
    pub team_manager: Arc<RwLock<TeammateManager>>,
}

impl ListTeammatesTool {
    pub fn new(team_manager: Arc<RwLock<TeammateManager>>,) -> Self {
        Self { team_manager }
    }
}

#[abu_tool::tool(
    struct_name = ListTeammatesTool,
    name = "list_teammates",
    description = "List all teammates with name, role, status."
)] 
pub async fn spawn_teammate(&self) -> String {
    self.team_manager.read().await.list_all().await
}

pub struct SendMessageTool {
    pub from: String,
    pub bus: Arc<MessageBus>,
}

impl SendMessageTool {
    pub fn new(from: impl Into<String>, bus: Arc<MessageBus>,) -> Self {
        Self { from: from.into(), bus }
    }
}

#[abu_tool::tool(
    struct_name = SendMessageTool,
    name = "send_message",
    description = "Send a message to a teammate's inbox."
)] 
pub async fn send_message(&self, to: &str, content: String, kind: MessageKind) -> anyhow::Result<String> {
    self.bus.send(&self.from, to, content, kind)
}

pub struct ReadInboxTool {
    pub name: String,
    pub bus: Arc<MessageBus>,
}

impl ReadInboxTool {
    pub fn new(name: impl Into<String>, bus: Arc<MessageBus>,) -> Self {
        Self { name: name.into(), bus }
    }
}

#[abu_tool::tool(
    struct_name = ReadInboxTool,
    name = "read_inbox",
    description = "Read and drain self inbox."
)] 
pub async fn read_inbox(&self) -> anyhow::Result<String> {
    let msg = self.bus.read_inbox(&self.name)?;
    Ok(serde_json::to_string_pretty(&msg)?)
}

// ====================================================================== //
//                      TeammateManager
// ====================================================================== //

#[derive(Serialize, Deserialize)]
pub struct TeammateConfig {
    pub team_name: String,
    pub members: Vec<TeammateMember>,
}

#[derive(Serialize, Deserialize)]
pub struct TeammateMember {
    pub name: String,
    pub status: TeammateMemberStatus,
    pub role: String,
}

#[abu_tool::tool_argument]
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TeammateMemberStatus {
    Idle,
    Shutdown,
    Working,
}

impl TeammateMemberStatus {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Idle => "idel",
            Self::Shutdown => "shutdown",
            Self::Working => "working",
        }
    }
}

pub struct TeammateManager {
    pub dir: PathBuf,
    pub config_path: PathBuf,
    pub config: Arc<RwLock<TeammateConfig>>,
    pub message_bus: Arc<MessageBus>,
    pub threads: HashMap<String, JoinHandle<anyhow::Result<()>>>,
}

const CONFIG_NAME: &'static str = "config.json";

impl TeammateManager {
    pub fn new(message_bus: Arc<MessageBus>) -> anyhow::Result<Self> {
        let team_dir: PathBuf = message_bus.dir.clone();
        let config_path = team_dir.join(CONFIG_NAME);
    
        let config: TeammateConfig = if std::fs::exists(&config_path)? {
            let content = std::fs::read_to_string(&config_path)?;
            serde_json::from_str(&content)?
        } else {
            TeammateConfig { team_name: "default".to_string(), members: vec![] } 
        };

        Ok(Self {
            dir: team_dir,
            config_path,
            config: Arc::new(RwLock::new(config)),
            message_bus,
            threads: HashMap::new(),
        })
    }
    
    pub async fn spawn(&mut self, name: &str, role: &str, prompt: &str) -> anyhow::Result<String> {
        {
            let mut config = self.config.write().await;
            if let Some(m) = config.find_member_mut(name) {
                // 不能召唤正在工作的员工
                if m.status != TeammateMemberStatus::Idle || m.status != TeammateMemberStatus::Shutdown {
                    return Ok(format!("Error: '{}' is currently {}", name, m.status.to_str()))
                }
                m.status = TeammateMemberStatus::Working;
                m.role = role.to_string();
            } else {
                let m = TeammateMember { name: name.to_string(), role: role.to_string(), status: TeammateMemberStatus::Working, };
                config.members.push(m);
            }
            config.save(&self.config_path)?;
        }

        let mut agent = self.new_agent(name, role).await?;
        let config = self.config.clone();
        let config_path = self.config_path.clone();
        let name_str = name;
        let name = name.to_string();
        let prompt = prompt.to_string();
        let handle: JoinHandle<anyhow::Result<()>> = tokio::spawn(async move {
            agent.run(&prompt).await?;
            let mut config = config.write().await;
            let member = config.find_member_mut(&name).expect("should be exit member");
            if member.status != TeammateMemberStatus::Idle {
                member.status = TeammateMemberStatus::Idle;
                config.save(&config_path)?;
            }
            Ok(())
        });

        self.threads.insert(name_str.to_string(), handle);
        Ok(format!("Spawned '{name_str}' (role: {role})"))
    }

    pub async fn list_all(&self) -> String {
        let config = self.config.read().await;
        if config.members.is_empty() {
            "No teammates.".to_string()
        } else {
            let mut lines = vec![format!("Team: {}", config.team_name)];
            for m in config.members.iter() {
                lines.push(format!("- {} ({}): {}", m.name, m.role, m.status.to_str()));
            }
            lines.join("\n")
        }
    }

    async fn new_agent(&self, name: &str, role: &str) -> anyhow::Result<Agent<DeepSeek, NoMemory, NoContextCompact>> {
        let model = ChatModel::deepseek("deepseek-chat")?;
        let cur_path = std::env::current_dir()?;
        let system_prompt = format!("You are '{name}', role: {role}, at {cur_path:?}. Use send_message to communicate. Complete your task.");
        let agent = AgentBuilder::new(model)
            .max_iteration(20)
            .system_prompt(system_prompt)
            .with_hook(ConsoleLoggerHook::new())
            .with_tool(Bash::new())
            .with_tool(ReadFile::new())
            .with_tool(WriteFile::new())
            .with_tool(SendMessageTool::new(name, self.message_bus.clone()))
            .with_tool(ReadInboxTool::new(name, self.message_bus.clone()))
            .with_llm_input_middleware(ReadInboxMiddleware::new(name, self.message_bus.clone()))
            .build().await?;
        Ok(agent)
    }

    async fn member_names(&self) -> Vec<String> {
        self.config.read().await.members.iter().map(|m| m.name.to_string()).collect()
    }
}

impl TeammateConfig {
    fn find_member_mut<'a, 'b>(&'a mut self, name: &'b str) -> Option<&'a mut TeammateMember>{
        for member in self.members.iter_mut() {
            if member.name == name {
                return Some(member)
            }
        }
        None
    }

    fn save(&self, config_path: &Path) -> anyhow::Result<()> {
        let contents = serde_json::to_string_pretty(&self)?;
        std::fs::write(config_path, contents)?;
        Ok(())
    }
}

// ====================================================================== //
//                      MessageBus
// ====================================================================== //

#[abu_tool::tool_argument]
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MessageKind {
    Message,
    Broadcast,
    ShutdownRequest,
    ShutdownResponse,
    PlanApproval,
    PlanApprovalResponse
}

impl MessageKind {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Message => "message",
            Self::Broadcast => "broadcast",
            Self::ShutdownRequest => "shutdown_request",
            Self::ShutdownResponse => "shutdown_response",
            Self::PlanApproval => "plan_approval",
            Self::PlanApprovalResponse => "plan_approval_response",
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub kind: MessageKind,
    pub from: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

pub struct MessageBus {
    dir: PathBuf,
}

impl MessageBus {
    pub fn new<P: Into<PathBuf>>(dir: P) -> anyhow::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// 创建 Message 对象，添加到 to 成员的 inbox 文件
    pub fn send(&self, 
        from: impl Into<String>, 
        to: &str, 
        content: impl Into<String>, 
        kind: MessageKind, 
    ) -> anyhow::Result<String> {
        let msg = Message {
            kind,
            from: from.into(),
            content: content.into(),
            timestamp: Utc::now(),
        };
        
        let inbox_path = self.inbox_path(to);
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(inbox_path)?;
        writeln!(file, "{}", serde_json::to_string(&msg)?)?;

        Ok(format!("Sent {} to {}", msg.kind.to_str(), to))
    }

    /// 读取 name 成员的 inbox 文件，返回 Message 列表，并且清空 inbox
    pub fn read_inbox(&self, name: &str) -> anyhow::Result<Vec<Message>> {
        let inbox_path = self.inbox_path(name);
        if !std::fs::exists(&inbox_path)? {
            return Ok(vec![])
        }

        let mut messages = vec![];
        for line in std::fs::read_to_string(&inbox_path)?.lines() {
            if line.is_empty() {
                continue;
            }
            let message: Message = serde_json::from_str(line)?;
            messages.push(message);
        }

        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&inbox_path)?;

        Ok(messages)        
    }

    /// 广播消息给多个队友
    pub fn broadcast<'a>(&'a self, from: &'a str, teammates: &[String], content: &'a str) -> anyhow::Result<String> {
        let mut count = 0;
        for to in teammates {
            if to != from {
                self.send(from, to, content, MessageKind::Broadcast)?;
                count += 1;
            }
        }
        Ok(format!("Broadcast to {count} teammates"))
    }

    /// 指定队员的邮箱
    fn inbox_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.jsonl"))
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
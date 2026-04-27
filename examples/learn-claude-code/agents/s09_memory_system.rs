use std::{collections::HashMap, io::Write, path::{Path, PathBuf}, process::Stdio, sync::{mpsc, Arc, OnceLock}, thread, time::Duration};
use abu_agent::{hook::ConsoleLoggerHook, middleware::{MiddlewareFlow, SystemPromptMiddleware}, model::ChatModel, AgentBuilder};
use regex::Regex;
use tokio::sync::RwLock;
use walkdir::WalkDir;
use std::process::Command;

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;

    let memory_manager = Arc::new(RwLock::new(MemoryManager::new(".memdir")));

    let model = ChatModel::deepseek("deepseek-chat")?;
    let cur_path = std::env::current_dir()?;
    println!("{:?}",cur_path);
    let mut agent = AgentBuilder::new(model)
        .system_prompt(format!("You are a coding agent at {:?}. Use tools to solve tasks.", cur_path))
        .with_hook(ConsoleLoggerHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool(MemoryTool::new(memory_manager.clone()))
        .with_system_prompt_middleware(MemoryMiddleware::new(memory_manager.clone()))
        .build().await?;

    println!("{:#?}", agent.tool_list());

    loop {
        print!("s09 >> ");
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

const MEMORY_GUIDANCE: &'static str = 
r#"When to save memories:
- User states a preference ("I like tabs", "always use pytest") -> type: user
- User corrects you ("don't do X", "that was wrong because...") -> type: feedback
- You learn a project fact that is not easy to infer from current code alone
  (for example: a rule exists because of compliance, or a legacy module must
  stay untouched for business reasons) -> type: project
- You learn where an external resource lives (ticket board, dashboard, docs URL)
  -> type: reference
When NOT to save:
- Anything easily derivable from code (function signatures, file structure, directory layout)
- Temporary task state (current branch, open PR numbers, current TODOs)
- Secrets or credentials (API keys, passwords)"#;

pub struct MemoryMiddleware {
    pub memory_manager: Arc<RwLock<MemoryManager>>,
}

impl MemoryMiddleware {
    pub fn new(memory_manager: Arc<RwLock<MemoryManager>>,) -> Self {
        Self { memory_manager }
    }
}

#[async_trait::async_trait]
impl SystemPromptMiddleware for MemoryMiddleware {
    type Error = anyhow::Error;
    async fn intercept(&mut self, prompt: &mut String) -> Result<MiddlewareFlow, Self::Error> {
        prompt.push_str(&self.memory_manager.read().await.load_memory_prompt());
        prompt.push_str("\n\n");
        prompt.push_str(MEMORY_GUIDANCE);
        Ok(MiddlewareFlow::Continue)
    }
}

// ====================================================================== //
//                      Wrap Tool
// ====================================================================== //

pub struct MemoryTool {
    pub memory_manager: Arc<RwLock<MemoryManager>>,
}

impl MemoryTool {
    pub fn new(memory_manager: Arc<RwLock<MemoryManager>>,) -> Self {
        Self { memory_manager }
    }
}

#[abu_tool::tool(
    struct_name = MemoryTool,
    name = "save_memory",
    description = "Save a persistent memory that survives across sessions."
)] 
pub async fn save_memory(&self,
    #[arg(description = "Short identifier (e.g. prefer_tabs, db_schema)")]
    name: &str,
    #[arg(description = "One-line summary of what this memory captures")]
    description: &str,
    #[arg(description = "user=preferences, feedback=corrections, project=non-obvious project conventions or decision reasons, reference=external resource pointers")]
    kind: MemoryKind,
    #[arg(description = "Full memory content (multi-line OK)")]
    content: &str,
) -> anyhow::Result<String> {
    self.memory_manager.write().await.save_memory(name, description, kind, content)
}

// ====================================================================== //
//                      Memory
// ====================================================================== //

pub struct Memory {
    pub name: String,
    pub description: String,
    pub kind: MemoryKind,
    pub content: String,
    pub file: String,
}

#[abu_tool::tool_argument]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum MemoryKind {
    User, Feedback, Project, Reference,
}

impl MemoryKind {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "user" => Self::User,
            "feedback" => Self::Feedback,
            "project" => Self::Project,
            "reference" => Self::Reference,
            _ => panic!("bad kind str"),
        }
    }
}

/// Load, build, and save persistent memories across sessions.
/// The teaching version keeps memory explicit:
/// one Markdown file per memory, plus one compact index file.
pub struct MemoryManager {
    pub memory_dir: PathBuf,
    pub memories: HashMap<String, Memory>,
}

impl MemoryManager {
    pub fn new<P: Into<PathBuf>>(memory_dir: P) -> Self {
        Self { memory_dir: memory_dir.into(), memories: HashMap::new() }
    }

    /// Load MEMORY.md index and all individual memory files.
    pub fn load_all(&mut self) -> anyhow::Result<()> {
        self.memories.clear();
        if !self.memory_dir.exists() {
            return Ok(())
        }

        // 遍历所有 .md 文件
        for file in WalkDir::new(&self.memory_dir)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
        {
            if file.file_name() == "MEMORY.md" {
                continue;
            }
            let context = std::fs::read_to_string(file.file_name())?;
            // 解析元信息
            if let Some(mem) = self.parse_frontmatter(&context) {
                self.memories.insert(mem.name.clone(), mem);
            }
        }

        Ok(())
    }

    /// 保存记忆并重建索引
    pub fn save_memory(&mut self, name: &str, description: &str, kind: MemoryKind, content: &str) -> anyhow::Result<String> {
        let safe_name = name.to_string()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect::<String>();
        
        // 创建 mem 目录
        std::fs::create_dir_all(&self.memory_dir)?;

        // 构造记录文件路径
        let file_name = format!("{}.md", safe_name);
        let file_path = self.memory_dir.join(&file_name);

        // 构建带有 Frontmatter 的内容
        let full_content = format!(
            "---\nname: {}\ndescription: {}\ntype: {}\n---\n{}\n",
            name, description, kind.to_str(), content
        );

        // 写入文件
        std::fs::write(&file_path, full_content)?;

        // 更新内存状态
        self.memories.insert(name.to_string(), Memory {
            name: name.to_string(),
            description: description.to_string(),
            kind,
            content: content.to_string(),
            file: file_name,
        });

        self.rebuild_index()?;

        Ok(format!("Saved memory '{}' [{}]", name, kind.to_str()))
    }

    /// 构建注入给 LLM 的 Prompt 部分
    pub fn load_memory_prompt(&self) -> String {
        if self.memories.is_empty() {
            return String::new();
        }

        let mut sections = vec![String::from("# Memories (persistent across sessions)\n")]; 
        // 提取出每种类型
        for kind in [MemoryKind::Project, MemoryKind::Feedback, MemoryKind::User, MemoryKind::Reference] {
            let typed_mems: Vec<&Memory> = self.memories.values()
                .filter(|m| m.kind == kind)
                .collect();
            if typed_mems.is_empty() { continue; }

            sections.push(format!("## [{}]", kind.to_str()));
            for mem in typed_mems {
                sections.push(format!("### {}: {}", mem.name, mem.description));
                if !mem.content.is_empty() {
                    sections.push(mem.content.clone());
                }
                sections.push(String::new());
            }
        }

        sections.join("\n")
    }

    /// 解析 Markdown 中的 Frontmatter
    fn parse_frontmatter(&self, text: &str) -> Option<Memory> {
        // 匹配 --- 包裹的元数据
        let re = Regex::new(r"(?s)^---\s*\n(.*?)\n---\s*\n(.*)").ok()?;
        let caps = re.captures(text)?;
        let header = caps.get(1)?.as_str();
        let body = caps.get(2)?.as_str().trim();

        let mut name = String::new();
        let mut description = String::new();
        let mut kind = MemoryKind::Project;

        for line in header.lines() {
            if let Some((key, value)) = line.split_once(':') {
                match key.trim() {
                    "name" => name = value.trim().to_string(),
                    "description" => description = value.trim().to_string(),
                    "kind" => kind = MemoryKind::from_str(&value.trim()),
                    _ => {}
                }
            }
        }

        if name.is_empty() { return None; }

        Some(Memory {
            name,
            description,
            kind,
            content: body.to_string(),
            file: String::new(), // 由调用者填充
        })
    } 

    /// 更新 MEMORY.md 索引文件
    fn rebuild_index(&self) -> anyhow::Result<()> {
        // 总标题
        let mut lines = vec![String::from("# Memory Index"), String::new()];
        
        // 对当前所哟记忆按照名称进行排序
        let mut sorted_names: Vec<_> = self.memories.keys().collect();
        sorted_names.sort();

        // 根据排序顺序插入
        for name in sorted_names {
            let mem = &self.memories[name];
            // 对每个记忆，不直接写内容
            lines.push(format!("- {}: {} [{}]", name, mem.description, mem.kind.to_str()));
            if lines.len() > 200 {
                lines.push(String::from("... (truncated)"));
                break;
            }
        }   

        // 写入 MEMORY.md
        let index_path = self.memory_dir.join("MEMORY.md");
        std::fs::write(index_path, lines.join("\n") + "\n")?;

        Ok(())
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
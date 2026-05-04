use std::{io::Write, path::{Path, PathBuf}, process::Stdio, sync::{mpsc, OnceLock}, thread, time::Duration};
use abu_agent::{hook::ConsoleLoggerHook, middleware::{MiddlewareFlow, SystemPromptMiddleware}, model::ChatModel, AgentBuilder};
use chrono::Utc;
use std::process::Command;

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;
    let builder = SystemPromptBuilder::new("./workspace")?;
    let model = ChatModel::deepseek("deepseek-chat")?;
    let cur_path = std::env::current_dir()?;
    println!("{:?}",cur_path);
    let mut agent = AgentBuilder::new(model)
        .system_prompt("")
        .with_system_prompt_middleware(builder)
        .with_hook(ConsoleLoggerHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_skills("./skills")
        .build().await?;

    loop {
        print!("s10 >> ");
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
//                      SystemPromptBuilder
// ====================================================================== //

#[async_trait::async_trait]
impl SystemPromptMiddleware for SystemPromptBuilder {
    type Error = anyhow::Error;
    async fn intercept(&mut self, prompt: &mut String) -> Result<MiddlewareFlow, Self::Error> {
        let sys_prompt = self.build()?;
        prompt.push_str(&sys_prompt);
        Ok(MiddlewareFlow::Continue)
    }
}

pub struct SystemPromptBuilder {
    pub workdir: PathBuf,
}

impl SystemPromptBuilder {
    pub fn new<P: Into<PathBuf>>(workdir: P) -> anyhow::Result<Self> {
        let workdir = workdir.into(); 
        Ok(Self { workdir })
    }

    pub fn build(&self) -> anyhow::Result<String> {
        let mut sections = vec![];
        sections.push(self.build_core());
        sections.push(self.build_claude_md()?);
        sections.push(self.build_dynamic_context()?);
        
        Ok(sections.join("\n\n"))
    }

    fn build_core(&self) -> String {
        format!(r#"
You are a coding agent operating in {:?}.
Use the provided tools to explore, reada and write files.
Always verify before assuming. Prefer reading files over guessing."#, self.workdir)
    }

    /// Load CLAUDE.md files in priority order (all are included):
    /// 1. ~/.claude/CLAUDE.md (user-global instructions)
    /// 2. <project-root>/CLAUDE.md (project instructions)
    /// 3. <current-subdir>/CLAUDE.md (directory-specific instructions)
    fn build_claude_md(&self) -> anyhow::Result<String> {
        let mut sources = vec![];

        // User-global
        let user_claude = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
        if user_claude.exists() {
            let user_claude_content = std::fs::read_to_string(&user_claude)?;
            sources.push((user_claude, user_claude_content));
        }

        // Project root
        let project_claude = self.workdir.join("CLAUDE.md");
        if project_claude.exists() {
            let project_claude_content = std::fs::read_to_string(&project_claude)?;
            sources.push((project_claude, project_claude_content));
        }

        if sources.is_empty() {
            return Ok("".to_string())
        }

        let mut parts = vec!["# CLAUDE.md instructions".to_string()];
        for (label, content) in sources {
            parts.push(format!("## From {label:?}"));
            parts.push(content);
        }

        Ok(parts.join("\n\n"))
    }

    fn build_dynamic_context(&self) -> anyhow::Result<String> {
        let lines = vec![
            "# Dynamic context\n".to_string(),
            format!("Current date: {}", Utc::now()),
            format!("Working directory: {:?}", self.workdir),
            format!("Platform: Linux"),
        ];
        Ok(lines.join("\n"))
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
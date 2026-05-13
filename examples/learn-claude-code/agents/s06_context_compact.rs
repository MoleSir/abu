use std::{io::Write, path::{Path, PathBuf}, process::Stdio, sync::{mpsc, OnceLock}, thread, time::Duration};
use abu_agent::{compact::ContextCompact, hook::ConsoleLoggerHook, middleware::{MiddlewareFlow, ToolResultMiddleware}, model::ChatModel, AgentBuilder, AgentContext};
use abu_provider::{chat::{ChatMessage, ToolCall}, ChatProvide};
use abu_tool::ToolCallResult;
use tokio::fs;
use std::process::Command;

#[tokio::main]
async fn main() {
    if let Err(e) = result_main().await {
        eprintln!("{:?}", e);
    } 
}

async fn result_main() -> anyhow::Result<()> {    
    dotenv::from_filename(".env")?;
    let model = ChatModel::deepseek("deepseek-chat")?;
    let compact = SummarizationCompact::new(model, 25);

    let model = ChatModel::deepseek("deepseek-chat")?;
    let cur_path = std::env::current_dir()?;
    println!("{:?}",cur_path);
    let mut agent = AgentBuilder::new(model)
        .system_prompt(format!("You are a coding agent at {:?}. Use bash to inspect and change the workspace. Act first, then report clearly.", cur_path))
        .compact(compact)
        .with_hook(ConsoleLoggerHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .with_tool_result_middleware(CompactToolResult::new(".tool_results")?)
        .build().await?;

    loop {
        print!("s06 >> ");
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
//                      summarize
// ====================================================================== //

const KEEP_RECENT_TOOL_RESULTS: usize = 3;

pub struct SummarizationCompact<P> {
    llm: ChatModel<P>,
    summary_threshold: usize,
}

impl<P: ChatProvide> SummarizationCompact<P> {
    pub fn new(llm: ChatModel<P>, summary_threshold: usize) -> Self {
        Self { 
            llm,
            summary_threshold,
        }
    }

    fn format_message(msg: &ChatMessage) -> String {
        format!("{}: {}", msg.role(), msg.content())
    }

    fn micro_compact(&mut self, session: &mut Vec<ChatMessage>) -> anyhow::Result<()> {
        let mut tool_msg_indexs = vec![];
        for (index, message) in session.iter().enumerate() {
            if let ChatMessage::Tool(_) = message {
                tool_msg_indexs.push(index);
            }
        }

        if tool_msg_indexs.len() <= KEEP_RECENT_TOOL_RESULTS {
            Ok(())
        } else {
            for i in tool_msg_indexs.into_iter().rev().skip(KEEP_RECENT_TOOL_RESULTS).rev() {
                let msg = &mut session[i];
                if let ChatMessage::Tool(msg) = msg {
                    if msg.content.len() <= 120 {
                        msg.content = "[Earlier tool result compacted. Re-run the tool if you need full detail.]".to_string();
                    }
                }
            }

            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl<P: ChatProvide> ContextCompact for SummarizationCompact<P> {
    type Error = anyhow::Error;

    async fn compact(&mut self, context: &mut AgentContext) -> Result<(), Self::Error> {
        self.micro_compact(&mut context.conversations)?;

        if context.conversations.len() + context.memory.len() + 1 > self.summary_threshold {
            return Ok(())
        }

        // collection all messages
        let buffer_text = context.conversations.iter()
            .map(|m| Self::format_message(m))
            .collect::<Vec<_>>()
            .join("\n");

        // send to llm
        let summarization_prompt = format!(
           "Summarize this conversation for continuity. Include:  \
            1) What was accomplished, 2) Current state, 3) Key decisions made. \
            Be concise but preserve critical details.\n\n{}",
            buffer_text
        );
        let messages = vec![
            ChatMessage::system("You are an expert summarization engine."),
            ChatMessage::user(summarization_prompt),
        ];
        let response = self.llm.chat(messages).await?.message;

        let mut session = vec![];
        session.push(ChatMessage::user(format!("[Conversation compressed]: {}", response.content)));
        session.push(ChatMessage::assistant("Understood. I have the context from the summary. Continuing.", []));

        context.conversations = session;

        Ok(())
    }
}

// ====================================================================== //
//                      Tool Output
// ====================================================================== //

pub struct CompactToolResult {
    pub cache_dir: PathBuf,
}

impl CompactToolResult {
    pub fn new<P: Into<PathBuf>>(cache_dir: P) -> anyhow::Result<Self> {
        let cache_dir = cache_dir.into();
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }
}

const PERSIST_THRESHOLD: usize = 3000;
const PREVIEW_CHARS: usize = 2000;

#[async_trait::async_trait]
impl ToolResultMiddleware for CompactToolResult {
    type Error = anyhow::Error;
    async fn intercept(&mut self, tool_call: &ToolCall, result: &mut ToolCallResult) -> Result<MiddlewareFlow, Self::Error> {
        if result.context.len() <= PERSIST_THRESHOLD {
            Ok(MiddlewareFlow::Continue)   
        } else {
            let stored_path = self.cache_dir.join(format!("{}.txt", tool_call.id));
            fs::write(&stored_path, &result.context).await?;
            let preview = &result.context[..PREVIEW_CHARS];
            let absolute_path = std::fs::canonicalize(&stored_path)?;

            let content = format!(
r#"<persisted-output>
Full output saved to: {absolute_path:?}
Preview:
{preview}
</persisted-output>"#
            );
            result.context = content;

            Ok(MiddlewareFlow::Continue)   
        }
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
use std::ffi::OsStr;
use std::process::Stdio;
use tokio::process::{Command, Child, ChildStdout};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::mpsc;
use async_trait::async_trait;

use crate::error::McpError;
use crate::McpResult;
use super::{McpMessage, McpTransport};

pub struct McpProcessTransport {
    reader: BufReader<ChildStdout>,
    writer_tx: mpsc::Sender<String>,
    child: Child,
}

impl McpProcessTransport {
    pub fn new<I, S>(cmd: S, args: I) -> McpResult<Self> 
    where 
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let cmd = cmd.as_ref();
        // 1. 构建命令
        let mut command = Command::new(cmd);
        command.args(args);

        // 2. 配置管道
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()); 

        // 3. 启动子进程
        let mut child = command.spawn().map_err(|e| {
            McpError::Transport(format!("Failed to spawn process '{}': {}", cmd.display(), e))
        })?;

        // 4. 获取管道的所有权
        let child_stdin = child.stdin.take().ok_or_else(|| {
            McpError::Transport("Failed to open child process stdin".to_string())
        })?;
        
        let child_stdout = child.stdout.take().ok_or_else(|| {
            McpError::Transport("Failed to open child process stdout".to_string())
        })?;

        // 5. 创建写入通道 (Agent -> Server)
        let (writer_tx, mut writer_rx) = mpsc::channel::<String>(32);

        // 6. 启动后台写入任务
        tokio::spawn(async move {
            let mut writer = BufWriter::new(child_stdin);
            
            while let Some(message) = writer_rx.recv().await {
                // 写入消息本体
                if let Err(e) = writer.write_all(message.as_bytes()).await {
                    eprintln!("Error writing to server process stdin: {}", e);
                    break;
                }
                
                // 补充换行符 (MCP 协议通常要求每条消息一行)
                if !message.ends_with('\n') {
                    if let Err(_) = writer.write_all(b"\n").await { 
                        break; 
                    }
                }
                
                // 必须 flush，否则数据可能滞留在缓冲区，Server 收不到
                if let Err(e) = writer.flush().await {
                    eprintln!("Error flushing to server process: {}", e);
                    break;
                }
            }
            // 循环结束意味着 channel 关闭或管道破裂，任务自动结束
        });

        Ok(Self {
            reader: BufReader::new(child_stdout),
            writer_tx,
            child,
        })
    }
}

#[async_trait]
impl McpTransport for McpProcessTransport {
    async fn send(&mut self, message: McpMessage) -> McpResult<()> {
        // 序列化消息为 JSON 字符串
        let json = serde_json::to_string(&message)
            .map_err(|err| McpError::Serialization(err.to_string()))?;

        // 发送到后台写入任务
        self.writer_tx.send(json).await
            .map_err(|err| McpError::Transport(format!("Failed to send message to process writer: {}", err)))
    }

    async fn receive(&mut self) -> McpResult<McpMessage> {
        let mut line = String::new();
        
        // 从子进程的 stdout 读取一行
        // 如果子进程关闭或崩溃，这里会返回 Ok(0) (EOF)
        match self.reader.read_line(&mut line).await {
            Ok(0) => {
                Err(McpError::Transport("Connection closed by remote process (EOF)".to_string()))
            }
            Ok(_) => {
                // 解析 JSON
                match serde_json::from_str(&line) {
                    Ok(parsed) => Ok(parsed),
                    Err(err) => Err(McpError::Serialization(format!("Invalid JSON from server: {}. Raw: {}", err, line)))
                }
            }
            Err(err) => {
                Err(McpError::Transport(format!("Failed to read from process: {}", err)))
            }
        }
    }

    async fn close(&mut self) -> McpResult<()> {
        // 尝试优雅关闭 stdin，这通常会给子进程发送 EOF 信号
        // 注意：由于 writer 在后台任务中，直接 drop sender 就会关闭 channel，
        // 进而导致后台任务退出并 drop child_stdin，触发 EOF。
        
        // 强制杀死子进程 (防止僵尸进程)
        match self.child.kill().await {
            Ok(_) => Ok(()),
            // 如果进程已经退出了，忽略错误
            Err(e) if e.kind() == std::io::ErrorKind::InvalidInput => Ok(()), 
            Err(e) => Err(McpError::Transport(format!("Failed to kill process: {}", e))),
        }
    }
}
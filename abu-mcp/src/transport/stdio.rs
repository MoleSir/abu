use crate::error::McpError;
use crate::McpResult;
use super::{McpMessage, McpTransport};
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct McpStdioTransport {
    reader: tokio::io::BufReader<tokio::io::Stdin>,
    writer_tx: tokio::sync::mpsc::Sender<String>,
}

impl Default for McpStdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl McpStdioTransport {
    pub fn new() -> Self {
        let (writer_tx, mut writer_rx) = tokio::sync::mpsc::channel::<String>(32);

        // 一个后台任务
        tokio::spawn(async move {
            // 获取标准输出
            let mut writer = tokio::io::BufWriter::new(tokio::io::stdout());
            // 等待通道的接受端口，有内容就往终端写入
            while let Some(message) = writer_rx.recv().await {
                if let Err(e) = writer.write_all(message.as_bytes()).await {
                    eprintln!("Error writing to stdout: {}", e);
                }
                if let Err(e) = writer.write_all(b"\n").await {
                    eprintln!("Error writing newline to stdout: {}", e);
                }
                if let Err(e) = writer.flush().await {
                    eprintln!("Error flushing stdout: {}", e);
                }
            }
        });

        Self {
            reader: tokio::io::BufReader::new(tokio::io::stdin()),
            writer_tx,
        }
    }
}

impl Clone for McpStdioTransport {
    fn clone(&self) -> Self {
        Self {
            reader: BufReader::new(tokio::io::stdin()),
            writer_tx: self.writer_tx.clone(),
        }
    }
}

#[async_trait]
impl McpTransport for McpStdioTransport {
    async fn send(&mut self, message: McpMessage) -> McpResult<()> {
        let json = serde_json::to_string(&message)
            .map_err(|err| McpError::Serialization(err.to_string()))?;

        self.writer_tx.send(json).await
            .map_err(|err| McpError::Transport(format!("Failed to send message to writer: {}", err)))
    }

    async fn receive(&mut self) -> McpResult<McpMessage> {
        let mut line = String::new();
        // 等待从标准输入获取输入
        match self.reader.read_line(&mut line).await {
            Ok(_) => match serde_json::from_str(&line) {
                Ok(parsed) => Ok(parsed),
                Err(err) => Err(McpError::Serialization(err.to_string()))
            }
            Err(err) => {
                Err(McpError::Transport(format!("Failed to read: {}", err)))
            }
        }
    }

    async fn close(&mut self) -> McpResult<()> {
        Ok(())
    }
}

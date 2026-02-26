use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use super::{McpMessage, McpResult, McpTransport};
use crate::McpError;

pub struct McpTcpTransport {
    reader: tokio::io::BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer_tx: tokio::sync::mpsc::Sender<String>,
}

impl McpTcpTransport {
    pub async fn new(stream: tokio::net::TcpStream) -> Self {
        let (reader, mut writer) = stream.into_split();
        let (writer_tx, mut writer_rx) = tokio::sync::mpsc::channel::<String>(32);

        // 一个后台任务
        tokio::spawn(async move {
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
            reader: tokio::io::BufReader::new(reader),
            writer_tx,
        }
    }
}

#[async_trait]
impl McpTransport for McpTcpTransport {
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

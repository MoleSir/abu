use chrono::Utc;
use serde::Serialize;
use std::path::Path;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::Mutex;
use super::{Hook, HookEvent};

#[derive(Serialize)]
struct AuditRecord<'a> {
    timestamp: String,
    #[serde(flatten)]
    event: &'a HookEvent<'a>,
}

pub struct AuditFileLoggerHook {
    writer: Mutex<BufWriter<File>>,
}

impl AuditFileLoggerHook {
    pub async fn new(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .create(true)   // 如果文件不存在则创建
            .append(true)   // 追加模式，防止覆盖历史日志
            .open(path)
            .await?;

        // 使用 BufWriter 减少频繁的系统调用 (默认 8KB 缓存)
        let writer = BufWriter::new(file);

        Ok(Self {
            writer: Mutex::new(writer),
        })
    }
}

#[async_trait::async_trait]
impl Hook for AuditFileLoggerHook {
    type Error = std::io::Error;

    async fn on_event(&self, event: &HookEvent<'_>) -> Result<(), Self::Error> {
        // 1. 组装带有时间戳的审计记录
        let record = AuditRecord {
            timestamp: Utc::now().to_rfc3339(),
            event,
        };

        // 2. 将事件序列化为单行 JSON 字符串 (JSONL 格式)
        let mut json_line = serde_json::to_string(&record).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to serialize event: {}", e),
            )
        })?;
        
        // 追加换行符
        json_line.push('\n');

        // 3. 异步获取锁并写入文件
        let mut writer = self.writer.lock().await;
        writer.write_all(json_line.as_bytes()).await?;
        writer.flush().await?;

        Ok(())
    }
}
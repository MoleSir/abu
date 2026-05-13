use std::path::Path;
use abu_provider::chat::ChatMessage;

/// Stateless manager for conversation JSONL I/O.
pub struct ConversationManager;

impl ConversationManager {
    pub fn new() -> Self {
        Self
    }

    /// Write messages to a JSONL file.
    pub fn save(&self, file_path: &Path, messages: &[ChatMessage]) -> anyhow::Result<()> {
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut content = String::new();
        for msg in messages {
            content.push_str(&serde_json::to_string(msg)?);
            content.push('\n');
        }
        std::fs::write(file_path, &content)?;
        Ok(())
    }

    /// Read messages from a JSONL file. Returns empty vec if the file doesn't exist.
    pub fn load(&self, file_path: &Path) -> anyhow::Result<Vec<ChatMessage>> {
        if !file_path.exists() {
            return Ok(vec![]);
        }
        let content = std::fs::read_to_string(file_path)?;
        let mut messages = vec![];
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            messages.push(serde_json::from_str(line)?);
        }
        Ok(messages)
    }

    /// Count non-empty lines in a conversation JSONL file.
    pub fn count_messages(&self, file_path: &Path) -> usize {
        std::fs::read_to_string(file_path)
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0)
    }
}

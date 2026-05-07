use std::path::{Path, PathBuf};

use anyhow::Context;
use abu_provider::chat::ChatMessage;
use chrono::Utc;

/// Manages session persistence: save/load conversation history as JSONL files.
///
/// Each session is stored as a JSONL file under the sessions directory,
/// named with a timestamp: `session_20260507_143021.jsonl`.
pub struct SessionManager {
    pub sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn new<P: Into<PathBuf>>(sessions_dir: P) -> anyhow::Result<Self> {
        let sessions_dir = sessions_dir.into();
        std::fs::create_dir_all(&sessions_dir)?;
        Ok(Self { sessions_dir })
    }

    /// Save the current session to a timestamped file.
    pub fn save(&self, session: &[ChatMessage]) -> anyhow::Result<PathBuf> {
        let now = Utc::now().format("%Y%m%d_%H%M%S");
        let file_name = format!("session_{}.jsonl", now);
        let file_path = self.sessions_dir.join(&file_name);

        let mut content = String::new();
        for msg in session {
            let line = serde_json::to_string(msg)?;
            content.push_str(&line);
            content.push('\n');
        }

        std::fs::write(&file_path, &content)?;
        Ok(file_path)
    }

    /// Load a session from a JSONL file.
    pub fn load(&self, path: &Path) -> anyhow::Result<Vec<ChatMessage>> {
        let content = std::fs::read_to_string(path)?;
        let mut messages = vec![];
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let msg: ChatMessage = serde_json::from_str(line)?;
            messages.push(msg);
        }
        Ok(messages)
    }

    /// List all session files, sorted by name (which is chronological).
    pub fn list_sessions(&self) -> anyhow::Result<Vec<PathBuf>> {
        let mut paths: Vec<PathBuf> = vec![];
        for entry in std::fs::read_dir(&self.sessions_dir)?.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "jsonl") {
                paths.push(path);
            }
        }
        paths.sort();
        Ok(paths)
    }

    /// Get the most recent session file, if any.
    pub fn latest_session(&self) -> anyhow::Result<Option<PathBuf>> {
        let sessions = self.list_sessions()?;
        Ok(sessions.into_iter().last())
    }

    /// Check if any session files exist.
    pub fn has_any_state(&self) -> bool {
        self.list_sessions().map(|s| !s.is_empty()).unwrap_or(false)
    }

    /// Delete all session files.
    pub fn clear_all(&self) -> anyhow::Result<()> {
        for path in self.list_sessions()? {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to remove session file {:?}", path))?;
        }
        Ok(())
    }

    /// Count messages in a session file.
    pub fn count_messages(&self, path: &Path) -> anyhow::Result<usize> {
        let content = std::fs::read_to_string(path)?;
        Ok(content.lines().filter(|l| !l.trim().is_empty()).count())
    }
}

mod conversation;
mod background;
mod todo;
pub mod wrap;
use std::path::PathBuf;
use abu_provider::chat::ChatMessage;
use anyhow::Context;

pub use conversation::ConversationManager;
pub use background::*;
pub use todo::*;

/// Manages session persistence: each session is a timestamp-named directory
/// under `sessions_dir` containing `conversation.jsonl`, `todos/`,
/// `background/`, and `tool_results/`.
///
/// Session IDs are timestamps: "20260508_143021" (YYYYMMDD_HHMMSS).
///
/// Owns all per-session state managers. `switch_session()` coordinates
/// saving the current conversation, switching to a different session,
/// reinitializing sub-managers, and loading the target conversation.
pub struct SessionManager {
    pub sessions_dir: PathBuf,
    current_session_id: String,
    pub conversation: ConversationManager,
    pub todo_manager: TodoManager,
    pub background_manager: BackgroundManager,
}

/// Lightweight session metadata for listing.
pub struct SessionInfo {
    pub id: String,
    pub message_count: usize,
    pub is_current: bool,
}

impl SessionManager {
    /// Create a new session manager, create a timestamp-named session directory,
    /// and initialize all per-session sub-managers. The new session is set as
    /// current. Returns the manager ready to use.
    pub fn new<P: Into<PathBuf>>(sessions_dir: P) -> anyhow::Result<Self> {
        let sessions_dir = sessions_dir.into();
        std::fs::create_dir_all(&sessions_dir)?;

        let id = loop {
            let id = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
            let dir = sessions_dir.join(&id);
            if !dir.exists() {
                break id;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        };
        let session_dir = sessions_dir.join(&id);
        std::fs::create_dir_all(&session_dir)?;

        let todos_dir = session_dir.join("todos");
        let background_dir = session_dir.join("background");

        Ok(Self {
            sessions_dir,
            current_session_id: id,
            conversation: ConversationManager::new(),
            todo_manager: TodoManager::new(&todos_dir)
                    .with_context(|| format!("Failed to init todos at {:?}", todos_dir))?,
            background_manager: BackgroundManager::new(&background_dir)
                    .with_context(|| format!("Failed to init background at {:?}", background_dir))?,
        })
    }

    // ── Delegation methods (convenience, delegate to self.conversation) ───

    /// Save the given messages to the current session's `conversation.jsonl`.
    pub async fn save_conversation(&self, messages: &[ChatMessage]) -> anyhow::Result<String> {
        let id = self.current_session_id();
        let file_path = self.conversation_file_for(&id);
        self.conversation
            .save(&file_path, messages)
            .with_context(|| format!("Failed to save conversation for session {}", id))?;
        Ok(id.to_string())
    }

    /// Load messages from a specific session's `conversation.jsonl`.
    pub async fn load_conversation(&self, session_id: &str) -> anyhow::Result<Vec<ChatMessage>> {
        let file_path = self.conversation_file_for(session_id);
        self.conversation
            .load(&file_path)
            .with_context(|| format!("Failed to load conversation for session {}", session_id))
    }

    // ── Session lifecycle ──────────────────────────────────────────────

    /// Fully switch to a different session:
    /// 1. Save current conversation (if any messages)
    /// 2. Update session ID
    /// 3. Reinitialize sub-managers for the target session
    /// 4. Load and return the target session's messages
    pub async fn switch_session(
        &mut self,
        id: &str,
        current_messages: &[ChatMessage],
    ) -> anyhow::Result<Vec<ChatMessage>> {
        if !current_messages.is_empty() {
            self.save_conversation(current_messages).await?;
        }

        self.current_session_id = id.to_string();
        let session_dir = self.session_dir_for(id);

        self.todo_manager
            .reinit(&session_dir.join("todos"))
            .with_context(|| format!("Failed to reinit todos for session {}", id))?;

        self.background_manager
            .reinit(&session_dir.join("background"))
            .with_context(|| format!("Failed to reinit background tasks for session {}", id))?;

        let tool_results_dir = session_dir.join("tool_results");
        std::fs::create_dir_all(&tool_results_dir)?;

        self.load_conversation(id).await
    }

    pub async fn list_todos(&self) -> anyhow::Result<String> {
        self.todo_manager.list_all()
    }

    /// List all sessions with metadata, sorted newest-first.
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionInfo>> {
        let current = self.current_session_id();
        let mut infos: Vec<SessionInfo> = vec![];

        for entry in std::fs::read_dir(&self.sessions_dir)?.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = path.file_name().unwrap().to_string_lossy().to_string();
            if !is_timestamp_dir(&dir_name) {
                continue;
            }
            let conv_file = path.join("conversation.jsonl");
            let message_count = self.conversation.count_messages(&conv_file);
            infos.push(SessionInfo {
                is_current: dir_name == current,
                message_count,
                id: dir_name,
            });
        }

        infos.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(infos)
    }

    /// Get the current session ID.
    pub fn current_session_id(&self) -> String {
        self.current_session_id.to_string()
    }

    pub fn current_session_dir(&self) -> PathBuf {
        self.sessions_dir.join(&self.current_session_id)
    }

    // ── private helpers ──────────────────────────────────────────────

    fn session_dir_for(&self, id: &str) -> PathBuf {
        self.sessions_dir.join(id)
    }

    fn conversation_file_for(&self, id: &str) -> PathBuf {
        self.session_dir_for(id).join("conversation.jsonl")
    }
}

fn is_timestamp_dir(name: &str) -> bool {
    if name.len() != 15 {
        return false;
    }
    let bytes = name.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if i == 8 {
            if b != b'_' {
                return false;
            }
        } else if !b.is_ascii_digit() {
            return false;
        }
    }
    true
}

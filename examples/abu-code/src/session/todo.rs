//! TODO system — batched, self-cleaning, with tool-level enforcement.
//!
//! TODOs are grouped into batches. Each batch corresponds to a single user
//! request. When all TODOs in a batch are completed, the batch auto-archives
//! and the next `todo_create` starts a fresh batch.
//!
//! **Enforcement**: `write_file` and `edit_file` refuse to execute if there are
//! no active TODOs. This forces the model to plan before acting.
//!
//! Directory layout:
//! ```text
//! todos/
//! ├── current_list_id
//! ├── 20260507_143021/
//! │   ├── 1.json
//! │   └── 2.json
//! └── 20260507_120000/       # archived (kept for history, not loaded)
//!     └── 1.json
//! ```

use std::{collections::HashSet, path::{Path, PathBuf}};

use anyhow::Context;
use serde::{Deserialize, Serialize};

const CURRENT_LIST_FILE: &str = "current_list_id";

// ============================================================================
// TODO model
// ============================================================================

#[abu_tool::tool_argument]
#[derive(Serialize, Clone, PartialEq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Deleted,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Todo {
    pub id: u32,
    pub subject: String,
    pub description: String,
    pub status: TodoStatus,
    #[serde(default)]
    pub blocked_by: Vec<u32>,
    #[serde(default)]
    pub blocks: Vec<u32>,
}

fn is_terminal(status: &TodoStatus) -> bool {
    matches!(status, TodoStatus::Completed | TodoStatus::Deleted)
}

// ============================================================================
// TodoManager
// ============================================================================

pub struct TodoManager {
    pub todos_dir: PathBuf,
    current_list_id: String,
    todos: Vec<Todo>,
    next_id: u32,
    terminal_ids: HashSet<u32>,
}

impl TodoManager {
    pub fn new<P: Into<PathBuf>>(todos_dir: P) -> anyhow::Result<Self> {
        let todos_dir = todos_dir.into();
        std::fs::create_dir_all(&todos_dir)?;

        let mut mgr = Self {
            todos_dir,
            current_list_id: String::new(),
            todos: Vec::new(),
            next_id: 1,
            terminal_ids: HashSet::new(),
        };

        mgr.load_or_create_list()?;
        Ok(mgr)
    }

    /// Reinitialize from a new todos directory (used when switching sessions).
    pub fn reinit(&mut self, todos_dir: &Path) -> anyhow::Result<()> {
        self.todos_dir = todos_dir.to_path_buf();
        self.current_list_id = String::new();
        self.todos.clear();
        self.next_id = 1;
        self.terminal_ids.clear();
        std::fs::create_dir_all(&self.todos_dir)?;
        self.load_or_create_list()?;
        Ok(())
    }

    pub fn has_any_state(&self) -> bool {
        !self.current_list_id.is_empty() && !self.todos.is_empty()
    }

    pub fn batch_id(&self) -> Option<&str> {
        if self.current_list_id.is_empty() {
            None
        } else {
            Some(&self.current_list_id)
        }
    }

    pub fn todo_counts(&self) -> (usize, usize, usize) {
        let pending = self.todos.iter().filter(|t| t.status == TodoStatus::Pending).count();
        let in_progress = self.todos.iter().filter(|t| t.status == TodoStatus::InProgress).count();
        let completed = self.todos.iter().filter(|t| t.status == TodoStatus::Completed).count();
        (pending, in_progress, completed)
    }

    // ---- list lifecycle ---------------------------------------------------

    fn load_or_create_list(&mut self) -> anyhow::Result<()> {
        let current_file = self.todos_dir.join(CURRENT_LIST_FILE);
        if current_file.exists() {
            let saved_id = std::fs::read_to_string(&current_file)?
                .trim()
                .to_string();
            if !saved_id.is_empty() {
                let list_dir = self.todos_dir.join(&saved_id);
                if list_dir.exists() {
                    let todos = Self::load_todos_from_dir(&list_dir)?;
                    let all_done = todos.iter().all(|t| is_terminal(&t.status));
                    if !all_done && !todos.is_empty() {
                        self.current_list_id = saved_id;
                        self.next_id = todos.iter().map(|t| t.id).max().unwrap_or(0) + 1;
                        self.terminal_ids = todos
                            .iter()
                            .filter(|t| is_terminal(&t.status))
                            .map(|t| t.id)
                            .collect();
                        self.todos = todos;
                        return Ok(());
                    }
                }
            }
        }

        self.new_list()
    }

    fn new_list(&mut self) -> anyhow::Result<()> {
        let list_id = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        self.current_list_id = list_id;
        self.todos.clear();
        self.next_id = 1;
        self.terminal_ids.clear();
        self.save_current_list_id()
    }

    fn archive_if_all_done(&mut self) -> anyhow::Result<()> {
        if self.todos.is_empty() {
            return Ok(());
        }
        if self.todos.iter().all(|t| is_terminal(&t.status)) {
            let current_file = self.todos_dir.join(CURRENT_LIST_FILE);
            std::fs::remove_file(&current_file)
                .with_context(|| format!("Failed to remove current list file {:?}", current_file))?;
            self.current_list_id.clear();
            self.todos.clear();
            self.next_id = 1;
            self.terminal_ids.clear();
        }
        Ok(())
    }

    fn save_current_list_id(&self) -> anyhow::Result<()> {
        let current_file = self.todos_dir.join(CURRENT_LIST_FILE);
        std::fs::write(&current_file, &self.current_list_id)?;
        Ok(())
    }

    fn list_dir(&self) -> PathBuf {
        self.todos_dir.join(&self.current_list_id)
    }

    // ---- CRUD -------------------------------------------------------------

    pub fn create(
        &mut self,
        subject: &str,
        description: &str,
        blocked_by: &[u32],
    ) -> anyhow::Result<String> {
        if self.current_list_id.is_empty() {
            self.new_list()?;
        }

        let id = self.next_id;
        self.next_id += 1;
        let list_dir = self.list_dir();
        std::fs::create_dir_all(&list_dir)?;

        let real_blockers: Vec<u32> = blocked_by
            .iter()
            .filter(|&&bid| bid > 0 && self.has_todo(bid))
            .copied()
            .collect();

        for &blocker_id in &real_blockers {
            let mut blocker = self.load(blocker_id)
                .with_context(|| format!("Failed to load blocker TODO #{}", blocker_id))?;
            if !blocker.blocks.contains(&id) {
                blocker.blocks.push(id);
                blocker.blocks.sort();
                blocker.blocks.dedup();
                self.save_todo(&list_dir, &blocker)?;
                if let Some(t) = self.todos.iter_mut().find(|t| t.id == blocker_id) {
                    t.blocks = blocker.blocks.clone();
                }
            }
        }

        let todo = Todo {
            id,
            subject: subject.to_string(),
            description: description.to_string(),
            status: TodoStatus::Pending,
            blocked_by: real_blockers,
            blocks: vec![],
        };

        self.save_todo(&list_dir, &todo)?;
        self.todos.push(todo.clone());
        self.save_current_list_id()?;

        Ok(serde_json::to_string_pretty(&todo)?)
    }

    pub fn set_status(&mut self, todo_id: u32, status: TodoStatus) -> anyhow::Result<()> {
        let mut todo = self.load(todo_id)?;
        todo.status = status.clone();
        self.save_todo(&self.list_dir(), &todo)?;

        if let Some(t) = self.todos.iter_mut().find(|t| t.id == todo_id) {
            t.status = status.clone();
        }

        if is_terminal(&status) {
            self.terminal_ids.insert(todo_id);
        } else {
            self.terminal_ids.remove(&todo_id);
        }

        self.archive_if_all_done()?;
        Ok(())
    }

    pub fn add_blocks(&mut self, todo_id: u32, blocked_id: u32) -> anyhow::Result<()> {
        if blocked_id == 0 {
            return Ok(());
        }
        let mut todo = self.load(todo_id)?;
        if !todo.blocks.contains(&blocked_id) {
            todo.blocks.push(blocked_id);
            todo.blocks.sort();
            todo.blocks.dedup();
        }
        self.save_todo(&self.list_dir(), &todo)?;
        if let Some(t) = self.todos.iter_mut().find(|t| t.id == todo_id) {
            t.blocks = todo.blocks.clone();
        }

        let mut other = self.load(blocked_id)
            .with_context(|| format!("Failed to load blocked TODO #{}", blocked_id))?;
        if !other.blocked_by.contains(&todo_id) {
            other.blocked_by.push(todo_id);
            other.blocked_by.sort();
            other.blocked_by.dedup();
            self.save_todo(&self.list_dir(), &other)?;
            if let Some(t) = self.todos.iter_mut().find(|t| t.id == blocked_id) {
                t.blocked_by = other.blocked_by.clone();
            }
        }
        Ok(())
    }

    pub fn add_blocked_by(&mut self, todo_id: u32, blocker_id: u32) -> anyhow::Result<()> {
        if blocker_id == 0 {
            return Ok(());
        }
        let mut todo = self.load(todo_id)?;
        if !todo.blocked_by.contains(&blocker_id) {
            todo.blocked_by.push(blocker_id);
            todo.blocked_by.sort();
            todo.blocked_by.dedup();
        }
        self.save_todo(&self.list_dir(), &todo)?;
        if let Some(t) = self.todos.iter_mut().find(|t| t.id == todo_id) {
            t.blocked_by = todo.blocked_by.clone();
        }

        let mut other = self.load(blocker_id)
            .with_context(|| format!("Failed to load blocker TODO #{}", blocker_id))?;
        if !other.blocks.contains(&todo_id) {
            other.blocks.push(todo_id);
            other.blocks.sort();
            other.blocks.dedup();
            self.save_todo(&self.list_dir(), &other)?;
            if let Some(t) = self.todos.iter_mut().find(|t| t.id == blocker_id) {
                t.blocks = other.blocks.clone();
            }
        }
        Ok(())
    }

    pub fn load(&self, todo_id: u32) -> anyhow::Result<Todo> {
        if let Some(t) = self.todos.iter().find(|t| t.id == todo_id) {
            return Ok(t.clone());
        }
        let path = self.list_dir().join(format!("{}.json", todo_id));
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn has_todo(&self, todo_id: u32) -> bool {
        self.todos.iter().any(|t| t.id == todo_id)
    }

    fn save_todo(&self, list_dir: &Path, todo: &Todo) -> anyhow::Result<()> {
        std::fs::create_dir_all(list_dir)?;
        let path = list_dir.join(format!("{}.json", todo.id));
        let content = serde_json::to_string_pretty(todo)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    fn load_todos_from_dir(dir: &Path) -> anyhow::Result<Vec<Todo>> {
        let mut todos: Vec<Todo> = vec![];
        for entry in std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read todo dir {:?}", dir))?
            .flatten()
        {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read todo file {:?}", path))?;
                let todo = serde_json::from_str::<Todo>(&content)
                    .with_context(|| format!("Failed to parse todo file {:?}", path))?;
                todos.push(todo);
            }
        }
        todos.sort_by_key(|t| t.id);
        Ok(todos)
    }

    // ---- queries ----------------------------------------------------------

    pub fn list_all(&self) -> anyhow::Result<String> {
        if self.todos.is_empty() {
            return Ok("No TODOs in current batch.".to_string());
        }

        let mut lines = vec![format!("Batch: {}", self.current_list_id)];
        for todo in &self.todos {
            let marker = match todo.status {
                TodoStatus::Pending => "[ ]",
                TodoStatus::InProgress => "[>]",
                TodoStatus::Completed => "[x]",
                TodoStatus::Deleted => "[-]",
            };

            let dep_info = if !todo.blocked_by.is_empty() {
                let active: Vec<_> = todo
                    .blocked_by
                    .iter()
                    .filter(|&&id| {
                        self.todos
                            .iter()
                            .any(|t| t.id == id && !is_terminal(&t.status))
                    })
                    .collect();
                if !active.is_empty() {
                    let ids: Vec<String> = active.iter().map(|id| format!("#{}", id)).collect();
                    format!(" (blocked by {})", ids.join(", "))
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            lines.push(format!("{} #{}: {}{}", marker, todo.id, todo.subject, dep_info));
        }
        Ok(lines.join("\n"))
    }

    #[allow(dead_code)]
    pub fn load_all(&self) -> anyhow::Result<Vec<Todo>> {
        Ok(self.todos.clone())
    }

    pub fn status_summary(&self) -> String {
        let active: Vec<&Todo> = self
            .todos
            .iter()
            .filter(|t| !is_terminal(&t.status))
            .collect();

        if active.is_empty() {
            return "\
[NO ACTIVE TODOS] You MUST create TODOs with todo_create BEFORE using \
write_file or edit_file. Break the user's request into at least 2 subtasks. \
These tools will REJECT your call if no TODOs exist."
                .to_string();
        }

        let pending = active.iter().filter(|t| t.status == TodoStatus::Pending).count();
        let in_progress = active
            .iter()
            .filter(|t| t.status == TodoStatus::InProgress)
            .count();

        let mut lines = vec![format!(
            "[TODO batch {} — {} pending, {} in_progress]",
            self.current_list_id, pending, in_progress
        )];

        for todo in &active {
            let marker = match todo.status {
                TodoStatus::InProgress => "[>]",
                _ => "[ ]",
            };
            let blocked = if !todo.blocked_by.is_empty() {
                let still_blocked: Vec<_> = todo
                    .blocked_by
                    .iter()
                    .filter(|&&id| {
                        !self.todos.iter().any(|t| t.id == id && t.status == TodoStatus::Completed)
                    })
                    .collect();
                if !still_blocked.is_empty() {
                    let ids: Vec<String> =
                        still_blocked.iter().map(|id| format!("#{}", id)).collect();
                    format!(" (blocked by {})", ids.join(", "))
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            lines.push(format!("{} #{}: {}{}", marker, todo.id, todo.subject, blocked));
        }

        lines.join("\n")
    }

    #[allow(dead_code)]
    pub fn cleanup_old_lists(&self, keep: usize) -> anyhow::Result<()> {
        let mut list_dirs: Vec<PathBuf> = vec![];
        for entry in std::fs::read_dir(&self.todos_dir)?.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path
                    .file_name()
                    .map_or(false, |n| n.to_string_lossy().contains('_'))
            {
                list_dirs.push(path);
            }
        }
        list_dirs.sort();
        let to_remove = list_dirs.len().saturating_sub(keep + 1);
        for dir in list_dirs.iter().take(to_remove) {
            if Some(dir) != self.list_dir().parent().map(|_| dir) {
                std::fs::remove_dir_all(dir)
                    .with_context(|| format!("Failed to remove old list dir {:?}", dir))?;
            }
        }
        Ok(())
    }
}


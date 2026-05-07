use std::{path::PathBuf, sync::Arc};

use serde::{Deserialize, Serialize};

// ============================================================================
// Task tools
// ============================================================================

pub struct TaskCreateTool(pub Arc<TaskManager>);
impl TaskCreateTool {
    pub fn new(t: Arc<TaskManager>) -> Self {
        Self(t)
    }
}

#[abu_tool::tool(
    struct_name = TaskCreateTool,
    name = "task_create",
    description = "Create a new task for tracking work."
)]
pub async fn task_create(&self, subject: &str, description: &str) -> anyhow::Result<String> {
    self.0.create(subject, description)
}

pub struct TaskUpdateTool(pub Arc<TaskManager>);
impl TaskUpdateTool {
    pub fn new(t: Arc<TaskManager>) -> Self {
        Self(t)
    }
}

#[abu_tool::tool(
    struct_name = TaskUpdateTool,
    name = "task_update",
    description = "Update a task's status. Statuses: pending -> in_progress -> completed. Use deleted to remove."
)]
pub async fn task_update(&self, task_id: u32, status: TaskStatus) -> anyhow::Result<String> {
    self.0.update(task_id, status)
}

pub struct TaskListTool(pub Arc<TaskManager>);
impl TaskListTool {
    pub fn new(t: Arc<TaskManager>) -> Self {
        Self(t)
    }
}

#[abu_tool::tool(
    struct_name = TaskListTool,
    name = "task_list",
    description = "List all tasks with their status."
)]
pub async fn task_list(&self) -> anyhow::Result<String> {
    self.0.list_all()
}

pub struct TaskGetTool(pub Arc<TaskManager>);
impl TaskGetTool {
    pub fn new(t: Arc<TaskManager>) -> Self {
        Self(t)
    }
}

#[abu_tool::tool(
    struct_name = TaskGetTool,
    name = "task_get",
    description = "Get full details of a task by ID."
)]
pub async fn task_get(&self, task_id: u32) -> anyhow::Result<String> {
    self.0.get(task_id)
}

// ============================================================================
// Task model
// ============================================================================

#[abu_tool::tool_argument]
#[derive(Serialize, Clone)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Deleted,
}

#[derive(Serialize, Deserialize)]
pub struct Task {
    pub id: u32,
    pub subject: String,
    pub description: String,
    pub status: TaskStatus,
}

// ============================================================================
// TaskManager
// ============================================================================

pub struct TaskManager {
    pub tasks_dir: PathBuf,
}

impl TaskManager {
    pub fn new<P: Into<PathBuf>>(tasks_dir: P) -> anyhow::Result<Self> {
        let tasks_dir = tasks_dir.into();
        std::fs::create_dir_all(&tasks_dir)?;
        Ok(Self { tasks_dir })
    }

    pub fn create(&self, subject: &str, description: &str) -> anyhow::Result<String> {
        let task_id = self.next_id()? + 1;
        let task = Task {
            id: task_id,
            subject: subject.to_string(),
            description: description.to_string(),
            status: TaskStatus::Pending,
        };
        self.save(&task)
    }

    pub fn update(&self, task_id: u32, status: TaskStatus) -> anyhow::Result<String> {
        let mut task = self.load(task_id)?;
        task.status = status;
        self.save(&task)
    }

    pub fn get(&self, task_id: u32) -> anyhow::Result<String> {
        let task = self.load(task_id)?;
        Ok(serde_json::to_string_pretty(&task)?)
    }

    pub fn list_all(&self) -> anyhow::Result<String> {
        let mut tasks: Vec<Task> = vec![];

        for entry in std::fs::read_dir(&self.tasks_dir)?.flatten() {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if file_name.starts_with("task_") && file_name.ends_with(".json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(task) = serde_json::from_str::<Task>(&content) {
                        tasks.push(task);
                    }
                }
            }
        }

        if tasks.is_empty() {
            return Ok("No tasks".to_string());
        }

        tasks.sort_by_key(|t| t.id);

        let mut lines = vec![];
        for task in tasks {
            let marker = match task.status {
                TaskStatus::Pending => "[ ]",
                TaskStatus::InProgress => "[>]",
                TaskStatus::Completed => "[x]",
                TaskStatus::Deleted => "[-]",
            };
            lines.push(format!("{} #{}: {}", marker, task.id, task.subject));
        }
        Ok(lines.join("\n"))
    }

    fn save(&self, task: &Task) -> anyhow::Result<String> {
        let path = self.tasks_dir.join(format!("task_{}.json", task.id));
        let content = serde_json::to_string_pretty(task)?;
        std::fs::write(&path, &content)?;
        Ok(content)
    }

    fn load(&self, task_id: u32) -> anyhow::Result<Task> {
        let path = self.tasks_dir.join(format!("task_{}.json", task_id));
        let content = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn next_id(&self) -> anyhow::Result<u32> {
        let mut max_id = 0u32;

        for entry in std::fs::read_dir(&self.tasks_dir)?.flatten() {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if file_name.starts_with("task_") && file_name.ends_with(".json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(id_part) = stem.split('_').nth(1) {
                        if let Ok(id) = id_part.parse::<u32>() {
                            max_id = max_id.max(id);
                        }
                    }
                }
            }
        }

        Ok(max_id)
    }
}

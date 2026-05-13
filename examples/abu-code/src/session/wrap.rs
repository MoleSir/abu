

// ============================================================================
// Background tools
// ============================================================================

use std::sync::Arc;

use abu_agent::middleware::{LlmInputMiddleware, MiddlewareFlow, SystemPromptMiddleware};
use abu_provider::chat::ChatMessage;
use anyhow::Context;
use tokio::sync::RwLock;

use super::{SessionManager, TodoStatus};

pub struct BackgroundRunTool {
    pub manager: Arc<RwLock<SessionManager>>,
}

impl BackgroundRunTool {
    pub fn new(manager: Arc<RwLock<SessionManager>>) -> Self {
        Self { manager }
    }
}

#[abu_tool::tool(
    struct_name = BackgroundRunTool,
    name = "background_run",
    description = "Run a shell command in the background. Returns a task ID immediately. Use background_check or background_list to track progress."
)]
pub async fn background_run(&self, command: String) -> String {
    let id = self.manager.write().await.background_manager.spawn(&command);
    format!("Background task started: {}\nCommand: {}", id, command)
}

pub struct BackgroundCheckTool {
    pub manager: Arc<RwLock<SessionManager>>,
}

impl BackgroundCheckTool {
    pub fn new(manager: Arc<RwLock<SessionManager>>) -> Self {
        Self { manager }
    }
}

#[abu_tool::tool(
    struct_name = BackgroundCheckTool,
    name = "background_check",
    description = "Check the status of a background task by its ID."
)]
pub async fn background_check(&self, task_id: String) -> String {
    self.manager.write().await.background_manager.check(&task_id)
}

pub struct BackgroundListTool {
    pub manager: Arc<RwLock<SessionManager>>,
}

impl BackgroundListTool {
    pub fn new(manager: Arc<RwLock<SessionManager>>) -> Self {
        Self { manager }
    }
}

#[abu_tool::tool(
    struct_name = BackgroundListTool,
    name = "background_list",
    description = "List all background tasks and their status.",
    category = "safe"
)]
pub async fn background_list(&self) -> String {
    self.manager.read().await.background_manager.list_all()
}

// ============================================================================
// BackgroundMiddleware — injects completion notifications into LLM input
// ============================================================================

pub struct BackgroundMiddleware {
    pub manager: Arc<RwLock<SessionManager>>,
}

impl BackgroundMiddleware {
    pub fn new(manager: Arc<RwLock<SessionManager>>) -> Self {
        Self { manager }
    }
}

#[async_trait::async_trait]
impl LlmInputMiddleware for BackgroundMiddleware {
    type Error = anyhow::Error;

    async fn intercept(
        &mut self,
        messages: &mut Vec<ChatMessage>,
    ) -> Result<MiddlewareFlow, Self::Error> {
        let mut manager = self.manager.write().await;
        manager.background_manager.check_completed()
            .with_context(|| "Failed to check for completed background tasks")?;
        let notifications = manager.background_manager.drain_notifications();

        for notification in notifications {
            messages.push(ChatMessage::user(notification));
        }

        Ok(MiddlewareFlow::Continue)
    }
}


// ============================================================================
// TODO tools
// ============================================================================

pub struct TodoCreateTool(pub Arc<RwLock<SessionManager>>);
impl TodoCreateTool {
    pub fn new(t: Arc<RwLock<SessionManager>>) -> Self {
        Self(t)
    }
}

#[abu_tool::tool(
    struct_name = TodoCreateTool,
    name = "todo_create",
    description = "Create a new TODO. Use blocked_by for dependency ordering. You MUST create TODOs before using write_file/edit_file."
)]
pub async fn todo_create(
    &self,
    subject: String,
    description: String,
    #[arg(
        description = "TODO IDs that must be completed before this one",
        default = "vec![]",
    )]
    blocked_by: Vec<u32>,
) -> anyhow::Result<String> {
    self.0.write().await.todo_manager.create(&subject, &description, &blocked_by)
}

pub struct TodoUpdateTool(pub Arc<RwLock<SessionManager>>);
impl TodoUpdateTool {
    pub fn new(t: Arc<RwLock<SessionManager>>) -> Self {
        Self(t)
    }
}

#[abu_tool::tool(
    struct_name = TodoUpdateTool,
    name = "todo_update",
    description = "Update a TODO: set status (pending/in_progress/completed/deleted), or manage dependencies."
)]
pub async fn todo_update(
    &self,
    todo_id: u32,
    #[arg(description = "New status", default = "Option::None")]
    status: Option<TodoStatus>,
    #[arg(description = "TODO ID that this TODO blocks", default = "Option::None")]
    add_blocks: Option<u32>,
    #[arg(description = "TODO ID that blocks this TODO", default = "Option::None")]
    add_blocked_by: Option<u32>,
) -> anyhow::Result<String> {
    let mut mgr = self.0.write().await;
    if let Some(id) = add_blocks {
        mgr.todo_manager.add_blocks(todo_id, id)?;
    }
    if let Some(id) = add_blocked_by {
        mgr.todo_manager.add_blocked_by(todo_id, id)?;
    }
    if let Some(s) = status {
        mgr.todo_manager.set_status(todo_id, s)?;
    }
    let todo = mgr.todo_manager.load(todo_id)?;
    Ok(serde_json::to_string_pretty(&todo)?)
}

pub struct TodoListTool(pub Arc<RwLock<SessionManager>>);
impl TodoListTool {
    pub fn new(t: Arc<RwLock<SessionManager>>) -> Self {
        Self(t)
    }
}

#[abu_tool::tool(
    struct_name = TodoListTool,
    name = "todo_list",
    description = "List all TODOs in the current batch with status and dependencies.",
    category = "safe"
)]
pub async fn todo_list(&self) -> anyhow::Result<String> {
    self.0.read().await.todo_manager.list_all()
}

pub struct TodoGetTool(pub Arc<RwLock<SessionManager>>);
impl TodoGetTool {
    pub fn new(t: Arc<RwLock<SessionManager>>) -> Self {
        Self(t)
    }
}

#[abu_tool::tool(
    struct_name = TodoGetTool,
    name = "todo_get",
    description = "Get full details of a TODO by ID.",
    category = "safe"
)]
pub async fn todo_get(&self, todo_id: u32) -> anyhow::Result<String> {
    let todo = self.0.read().await.todo_manager.load(todo_id)?;
    Ok(serde_json::to_string_pretty(&todo)?)
}

// ============================================================================
// TodoMiddleware — injects TODO status at the TOP of the system prompt
// ============================================================================

pub struct TodoMiddleware {
    pub todo_manager: Arc<RwLock<SessionManager>>,
}

impl TodoMiddleware {
    pub fn new(todo_manager: Arc<RwLock<SessionManager>>) -> Self {
        Self { todo_manager }
    }
}

#[async_trait::async_trait]
impl SystemPromptMiddleware for TodoMiddleware {
    type Error = anyhow::Error;

    async fn intercept(
        &mut self,
        prompt: &mut String,
    ) -> Result<MiddlewareFlow, Self::Error> {
        let summary = self.todo_manager.read().await.todo_manager.status_summary();
        prompt.insert_str(0, &format!("# Current TODOs\n\n{}\n\n---\n\n", summary));
        Ok(MiddlewareFlow::Continue)
    }
}
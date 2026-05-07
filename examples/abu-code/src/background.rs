//! Background task execution — run long shell commands asynchronously,
//! with completion notifications injected into the LLM input on each step.

use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::Arc,
};
use abu_agent::middleware::{LlmInputMiddleware, MiddlewareFlow};
use abu_provider::chat::ChatMessage;
use anyhow::Context;
use chrono::Utc;
use tokio::{
    process::Command,
    sync::RwLock,
};

use crate::tools;

// ============================================================================
// BackgroundManager
// ============================================================================

#[allow(dead_code)]
pub struct BackgroundTask {
    pub id: String,
    pub command: String,
    pub started: String,
    pub finished: Option<String>,
    pub output: String,
    pub exit_code: Option<i32>,
}

pub struct BackgroundManager {
    pub tasks: HashMap<String, BackgroundTask>,
    pub log_dir: PathBuf,
    notifications: Vec<String>,
    counter: u32,
}

impl BackgroundManager {
    pub fn new<P: Into<PathBuf>>(log_dir: P) -> anyhow::Result<Self> {
        let log_dir = log_dir.into();
        std::fs::create_dir_all(&log_dir)?;
        let mut mgr = Self {
            tasks: HashMap::new(),
            log_dir,
            notifications: vec![],
            counter: 0,
        };
        mgr.load_existing_results()
            .with_context(|| "Failed to load existing background task results")?;
        Ok(mgr)
    }

    /// Scan log dir for task results from previous runs and load into memory.
    fn load_existing_results(&mut self) -> anyhow::Result<()> {
        let dir = std::fs::read_dir(&self.log_dir)
            .with_context(|| format!("Failed to read background log dir {:?}", self.log_dir))?;

        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let id = path.file_stem().unwrap().to_string_lossy().to_string();

                // Extract counter from id for ordering (format: bg_HHMMSS_N)
                if let Some(last_underscore) = id.rfind('_') {
                    if let Ok(n) = id[last_underscore + 1..].parse::<u32>() {
                        self.counter = self.counter.max(n);
                    }
                }

                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read background status file {:?}", path))?;
                let status = serde_json::from_str::<serde_json::Value>(&content)
                    .with_context(|| format!("Failed to parse background status file {:?}", path))?;

                let finished = status
                    .get("error")
                    .map(|_| "previous session (failed)")
                    .unwrap_or("previous session (completed)");

                let output = self
                    .log_dir
                    .join(format!("{}.log", id));
                let output_str = std::fs::read_to_string(&output)
                    .with_context(|| format!("Failed to read background log file {:?}", output))?;

                self.tasks.insert(
                    id.clone(),
                    BackgroundTask {
                        id,
                        command: "(from previous session)".to_string(),
                        started: String::new(),
                        finished: Some(finished.to_string()),
                        output: output_str,
                        exit_code: status.get("exit_code").and_then(|v| v.as_i64()).map(|c| c as i32),
                    },
                );
            }
        }
        Ok(())
    }

    /// Queue a notification to be injected into the LLM input on the next step.
    pub fn notify(&mut self, msg: String) {
        self.notifications.push(msg);
    }

    /// Drain all pending notifications.
    pub fn drain_notifications(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notifications)
    }

    /// Spawn a background command. Returns immediately with a task ID.
    pub fn spawn(&mut self, command: &str) -> String {
        self.counter += 1;
        let id = format!("bg_{}_{}", Utc::now().format("%H%M%S"), self.counter);
        let started = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        self.tasks.insert(
            id.clone(),
            BackgroundTask {
                id: id.clone(),
                command: command.to_string(),
                started,
                finished: None,
                output: String::new(),
                exit_code: None,
            },
        );

        let id_clone = id.clone();
        let cmd_str = command.to_string();
        let log_dir = self.log_dir.clone();
        let workdir = tools::get_workdir().clone();

        tokio::spawn(async move {
            let result = Command::new("sh")
                .arg("-c")
                .arg(&cmd_str)
                .current_dir(&workdir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await;

            let log_path = log_dir.join(format!("{}.log", id_clone));
            let mut output = String::new();

            match result {
                Ok(out) => {
                    let out_str = String::from_utf8_lossy(&out.stdout);
                    let err_str = String::from_utf8_lossy(&out.stderr);
                    output.push_str(&out_str);
                    if !err_str.is_empty() {
                        output.push_str("\n[stderr]\n");
                        output.push_str(&err_str);
                    }

                    // Write to log file
                    if let Err(e) = std::fs::write(&log_path, &output) {
                        eprintln!("Failed to write background task log {:?}: {}", log_path, e);
                    }

                    // Return exit code and output via a file-based notification
                    let status_path = log_dir.join(format!("{}.json", id_clone));
                    let status = serde_json::json!({
                        "id": id_clone,
                        "exit_code": out.status.code(),
                        "output_len": output.len(),
                        "output_preview": &output[..output.len().min(500)],
                    });
                    if let Err(e) = std::fs::write(&status_path, serde_json::to_string(&status).unwrap()) {
                        eprintln!("Failed to write background task status {:?}: {}", status_path, e);
                    }
                }
                Err(e) => {
                    output = format!("Failed to execute: {}", e);
                    if let Err(write_err) = std::fs::write(&log_path, &output) {
                        eprintln!("Failed to write background task log {:?}: {}", log_path, write_err);
                    }
                    let status_path = log_dir.join(format!("{}.json", id_clone));
                    let status = serde_json::json!({
                        "id": id_clone,
                        "error": e.to_string(),
                    });
                    if let Err(write_err) = std::fs::write(&status_path, serde_json::to_string(&status).unwrap()) {
                        eprintln!("Failed to write background task status {:?}: {}", status_path, write_err);
                    }
                }
            }
        });

        id
    }

    /// Check a specific task's status.
    pub fn check(&mut self, id: &str) -> String {
        // Try to load status from JSON file
        let status_path = self.log_dir.join(format!("{}.json", id));
        let content = match std::fs::read_to_string(&status_path) {
            Ok(c) => c,
            Err(_) => {
                // File doesn't exist or can't be read — task may still be running
                if let Some(task) = self.tasks.get(id) {
                    return format!(
                        "Task {} is running. Command: {}. Started: {}",
                        id, task.command, task.started
                    );
                }
                return format!("Task {} not found. It may have been cleaned up.", id);
            }
        };

        let status = match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(s) => s,
            Err(e) => return format!("Task {} has corrupted status file: {}", id, e),
        };

        if status.get("exit_code").is_some() || status.get("error").is_some() {
            // Task completed — load full output from log
            let log_path = self.log_dir.join(format!("{}.log", id));
            let output = match std::fs::read_to_string(&log_path) {
                Ok(o) => o,
                Err(e) => return format!("Task {} completed but failed to read log: {}", id, e),
            };
            let preview: String = output.chars().take(3000).collect();
            let msg = if output.len() > 3000 {
                format!(
                    "Task {} completed.\nOutput ({} bytes, truncated):\n{}",
                    id,
                    output.len(),
                    preview
                )
            } else {
                format!("Task {} completed.\nOutput:\n{}", id, output)
            };
            return msg;
        }

        // Check in-memory tasks
        if let Some(task) = self.tasks.get(id) {
            format!(
                "Task {} is running. Command: {}. Started: {}",
                id, task.command, task.started
            )
        } else {
            format!("Task {} not found. It may have been cleaned up.", id)
        }
    }

    /// Scan for completed tasks and inject notifications.
    pub fn check_completed(&mut self) -> anyhow::Result<()> {
        let entries = std::fs::read_dir(&self.log_dir)
            .with_context(|| format!("Failed to read background log dir {:?}", self.log_dir))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                let id = path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                // Skip if already processed
                if let Some(task) = self.tasks.get(&id) {
                    if task.finished.is_some() {
                        continue;
                    }
                }

                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to read background status file {:?}: {}", path, e);
                        continue;
                    }
                };

                let status = match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Failed to parse background status file {:?}: {}", path, e);
                        continue;
                    }
                };

                let finished = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let preview = status
                    .get("output_preview")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(no preview)");

                self.notify(format!(
                    "[Background task {} completed at {}]\nPreview: {}",
                    id, finished, preview
                ));

                // Mark as processed
                if let Some(task) = self.tasks.get_mut(&id) {
                    task.finished = Some(finished);
                }
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    pub fn completed_count(&self) -> usize {
        self.tasks.values().filter(|t| t.finished.is_some()).count()
    }

    pub fn any_tasks(&self) -> bool {
        !self.tasks.is_empty()
    }

    /// Delete all background task logs and reset in-memory state.
    pub fn clear(&mut self) -> anyhow::Result<()> {
        let entries = std::fs::read_dir(&self.log_dir)
            .with_context(|| format!("Failed to read background log dir {:?}", self.log_dir))?;
        for entry in entries.flatten() {
            std::fs::remove_file(entry.path())
                .with_context(|| format!("Failed to remove background file {:?}", entry.path()))?;
        }
        self.tasks.clear();
        self.notifications.clear();
        self.counter = 0;
        Ok(())
    }

    /// List all background tasks.
    pub fn list_all(&self) -> String {
        if self.tasks.is_empty() {
            return "No background tasks.".to_string();
        }

        let mut lines = vec![];
        for task in self.tasks.values() {
            let status = if task.finished.is_some() {
                "done"
            } else {
                "running"
            };
            lines.push(format!(
                "  [{}] {} — {} ({})",
                status, task.id, task.command, task.started
            ));
        }
        lines.join("\n")
    }
}

// ============================================================================
// Background tools
// ============================================================================

pub struct BackgroundRunTool {
    pub manager: Arc<RwLock<BackgroundManager>>,
}

impl BackgroundRunTool {
    pub fn new(manager: Arc<RwLock<BackgroundManager>>) -> Self {
        Self { manager }
    }
}

#[abu_tool::tool(
    struct_name = BackgroundRunTool,
    name = "background_run",
    description = "Run a shell command in the background. Returns a task ID immediately. Use background_check or background_list to track progress."
)]
pub async fn background_run(&self, command: String) -> String {
    let id = self.manager.write().await.spawn(&command);
    format!("Background task started: {}\nCommand: {}", id, command)
}

pub struct BackgroundCheckTool {
    pub manager: Arc<RwLock<BackgroundManager>>,
}

impl BackgroundCheckTool {
    pub fn new(manager: Arc<RwLock<BackgroundManager>>) -> Self {
        Self { manager }
    }
}

#[abu_tool::tool(
    struct_name = BackgroundCheckTool,
    name = "background_check",
    description = "Check the status of a background task by its ID."
)]
pub async fn background_check(&self, task_id: String) -> String {
    self.manager.write().await.check(&task_id)
}

pub struct BackgroundListTool {
    pub manager: Arc<RwLock<BackgroundManager>>,
}

impl BackgroundListTool {
    pub fn new(manager: Arc<RwLock<BackgroundManager>>) -> Self {
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
    self.manager.read().await.list_all()
}

// ============================================================================
// BackgroundMiddleware — injects completion notifications into LLM input
// ============================================================================

pub struct BackgroundMiddleware {
    pub manager: Arc<RwLock<BackgroundManager>>,
}

impl BackgroundMiddleware {
    pub fn new(manager: Arc<RwLock<BackgroundManager>>) -> Self {
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
        manager.check_completed()
            .with_context(|| "Failed to check for completed background tasks")?;
        let notifications = manager.drain_notifications();

        for notification in notifications {
            messages.push(ChatMessage::user(notification));
        }

        Ok(MiddlewareFlow::Continue)
    }
}

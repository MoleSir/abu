use std::{io::Write, path::Path};

use abu_agent::tool::{ExecutionMode, Matcher, PermissionManager, UserAuthorizer, UserResponse};

pub struct InputUserAuthorizer;

#[async_trait::async_trait]
impl UserAuthorizer for InputUserAuthorizer {
    async fn ask_user(
        &self,
        tool_name: &str,
        _arguments: &serde_json::Value,
        preview_reason: &str,
    ) -> UserResponse {
        println!("\n  [Permission] {tool_name}: {preview_reason}");
        print!("  Allow? [Y/n/always]: ");
        std::io::stdout().flush().unwrap();

        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return UserResponse::Yes; // Ctrl-D defaults to yes
        }

        match answer.trim().to_lowercase().as_str() {
            "n" | "no" => UserResponse::No,
            "always" => UserResponse::Always,
            _ => UserResponse::Yes, // empty or anything else = yes
        }
    }
}

pub fn build_permission(data_dir: &Path) -> PermissionManager {
    let permissions_file = data_dir.join("permissions.json");

    let mut pm = PermissionManager::new(ExecutionMode::Auto, InputUserAuthorizer)
        // Deny destructive commands
        .with_deny_if("bash", "command", Matcher::contains("rm -rf /"))
        .with_deny_if("bash", "command", Matcher::contains("sudo"))
        .with_deny_if("bash", "command", Matcher::contains("shutdown"))
        .with_deny_if("bash", "command", Matcher::contains("reboot"))
        // Read-only tools (also auto-approved by Auto mode via category)
        .with_allow("read_file")
        .with_allow("glob")
        .with_allow("grep")
        .with_allow("todo_list")
        .with_allow("todo_get")
        .with_allow("background_list")
        .with_allow("load_skill")
        // Task management — just organizing work, not destructive
        .with_allow("todo_create")
        .with_allow("todo_update")
        .with_allow("save_memory")
        // Subagents — each has its own permission context
        .with_allow("task")
        .with_allow("explore")
        .with_allow("plan")
        .with_permissions_file(permissions_file);

    if let Err(e) = pm.load_persisted_rules() {
        eprintln!("Warning: failed to load permission rules: {}", e);
    }

    pm
}

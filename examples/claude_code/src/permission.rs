use std::io::Write;

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
        print!("  Allow? (y/n/always): ");
        std::io::stdout().flush().unwrap();

        let mut answer = String::new();
        if std::io::stdin().read_line(&mut answer).is_err() {
            return UserResponse::No;
        }

        match answer.trim() {
            "always" => UserResponse::Always,
            "y" | "yes" => UserResponse::Yes,
            _ => UserResponse::No,
        }
    }
}

pub fn build_permission() -> PermissionManager {
    PermissionManager::new(ExecutionMode::Default, InputUserAuthorizer)
        .with_deny_if("bash", "command", Matcher::contains("rm -rf /"))
        .with_deny_if("bash", "command", Matcher::contains("sudo"))
        .with_deny_if("bash", "command", Matcher::contains("shutdown"))
        .with_deny_if("bash", "command", Matcher::contains("reboot"))
        .with_allow("read_file")
}

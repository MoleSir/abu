use std::{convert::Infallible, io::Write};
use abu_base::chat::ToolCall;
use abu_tool::ToolCallResult;
use super::{MiddlewareFlow, ToolCallMiddleware, ToolResultMiddleware};
use std::collections::HashSet;

// ====================================================================== //
//                    HitlMiddleware
// ====================================================================== //

pub struct HitlMiddleware {
    dangerous_tools: Vec<String>, 
}

impl HitlMiddleware {
    pub fn new<S: Into<String>>(tools: impl IntoIterator<Item = S>) -> Self {
        let tools = tools.into_iter()
            .map(|t| t.into())
            .collect();
        Self {
            dangerous_tools: tools,
        }
    }
}

#[async_trait::async_trait]
impl ToolCallMiddleware for HitlMiddleware {
    type Error = Infallible;

    async fn intercept(&self, tool_call: &mut ToolCall) -> Result<MiddlewareFlow, Self::Error> {
        if self.dangerous_tools.contains(&tool_call.name) {
            println!("⚠️ [HITL] AI 想要执行高危操作: {}", tool_call.name);
            println!("   参数: {}", tool_call.arguments);
            print!("   同意执行吗？(y/N/edit): ");
            std::io::stdout().flush().unwrap();

            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            let input = input.trim().to_lowercase();

            match input.as_str() {
                "y" | "yes" => {
                    println!("✅ 人类已批准。");
                    Ok(MiddlewareFlow::Continue)
                }
                "edit" => {
                    print!("   请输入新的 JSON 参数: ");
                    std::io::stdout().flush().unwrap();
                    let mut new_args = String::new();
                    std::io::stdin().read_line(&mut new_args).unwrap();
                    tool_call.arguments = new_args.trim().to_string();
                    Ok(MiddlewareFlow::Continue)
                }
                _ => {
                    println!("🚫 人类已拒绝执行该操作。");
                    Ok(MiddlewareFlow::Break("Rejected".to_string()))
                }
            }
        } else {
            Ok(MiddlewareFlow::Continue)
        }
    }
}

// ====================================================================== //
//                    ToolPermissionGuardMiddleware
// ====================================================================== //

pub struct ToolPermissionGuardMiddleware {
    forbidden_tools: HashSet<String>,
}

impl ToolPermissionGuardMiddleware {
    pub fn new(forbidden: &[&str]) -> Self {
        let forbidden_tools = forbidden.iter().map(|&s| s.to_string()).collect();
        Self { forbidden_tools }
    }
}

#[async_trait::async_trait]
impl ToolCallMiddleware for ToolPermissionGuardMiddleware {
    type Error = Infallible;

    async fn intercept(&self, tool_call: &mut ToolCall) -> Result<MiddlewareFlow, Self::Error> {
        if self.forbidden_tools.contains(&tool_call.name) {
            let error_msg = format!(
                "System Error: Permission denied to execute tool '{}'. Please do not try to use this tool.",
                tool_call.name
            );
            return Ok(MiddlewareFlow::Break(error_msg));
        }

        Ok(MiddlewareFlow::Continue)
    }
}

pub struct ResultTruncatorMiddleware {
    max_length: usize,
}

impl ResultTruncatorMiddleware {
    pub fn new(max_length: usize) -> Self {
        Self { max_length }
    }
}

// ====================================================================== //
//                    ToolPermissionGuardMiddleware
// ====================================================================== //

#[async_trait::async_trait]
impl ToolResultMiddleware for ResultTruncatorMiddleware {
    type Error = Infallible;

    async fn intercept(&self, _tool_name: &str, result: &mut ToolCallResult) -> Result<MiddlewareFlow, Self::Error> {
        let current_len = result.context.chars().count();

        if current_len > self.max_length {
            let truncated: String = result.context.chars().take(self.max_length).collect();            
            result.context = format!(
                "{}...\n[System]: Output was too long and has been truncated to {} characters.",
                truncated, self.max_length
            );
        }

        Ok(MiddlewareFlow::Continue)
    }
}
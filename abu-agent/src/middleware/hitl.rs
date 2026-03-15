use std::{convert::Infallible, io::Write};
use abu_base::chat::ToolCall;
use super::{MiddlewareFlow, ToolCallMiddleware};

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

    async fn intercept(&self, tool_call: &mut ToolCall) -> Result<MiddlewareFlow<String>, Self::Error> {
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
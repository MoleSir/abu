use super::{Hook, HookEvent};
pub struct ConsoleLoggerHook;

#[async_trait::async_trait]
impl Hook for ConsoleLoggerHook {
    type Error = std::convert::Infallible;
    
    async fn on_event(&self, event: HookEvent<'_>) -> Result<(), Self::Error> {
        match event {
            HookEvent::AgentStart { query } => {
                println!("🚀 [Agent Start] Task: {}", query);
            }
            HookEvent::ToolStart { tool_call, .. } => {
                println!("   🛠️  [Action] Calling tool `{}` with args: {}", tool_call.name, tool_call.arguments);
            }
            HookEvent::ToolEnd { result, .. } => {
                if result.is_error {
                    println!("   ❌ [Action Failed] {}", result.context);
                } else {
                    println!("   ✅ [Observation] {}", result.context);
                }
            }
            HookEvent::LlmEnd { message, .. } => {
                if !message.content.is_empty() {
                    println!("   🤖 [AI Thought] {}", message.content);
                }
            }
            _ => {}
        }
        Ok(())
    }
}
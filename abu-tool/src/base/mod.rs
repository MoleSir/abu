mod argument;
mod error;
mod param;
mod result;

use abu_base::chat::ToolDefinition;
pub use argument::*;
pub use error::*;
pub use param::*;
pub use result::*;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolCategory {
    Safe,
    Mutating,
}

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> String;
    fn description(&self) -> String;
    fn parameters(&self) -> Vec<ToolParameter>;
    async fn execute(&self, args: Value) -> ToolResult<ToolCallResult>;

    fn category(&self) -> ToolCategory {
        ToolCategory::Mutating 
    }

    fn to_function_define(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            schema: build_params_schema(&self.parameters())
        }
    }
}
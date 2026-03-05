use std::collections::HashMap;
use abu_api::chat::{ToolCall, ToolDefinition};
use super::{Tool, ToolCallResult, ToolError, ToolResult};

pub struct ToolRegister {
    tools: HashMap<&'static str , Box<dyn Tool>>,
}

impl ToolRegister {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn init<I: Iterator<Item = Box<dyn Tool>>>(tools: I) -> Self {
        Self {
            tools: tools.map(|tool| (tool.name(), tool)).collect()
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn get_tool(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn add_tool<T: Tool + 'static>(&mut self, tool: T) {
        let tool = Box::new(tool);
        self.add_tool_box(tool);
    }

    pub fn add_tool_box(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name();
        self.tools.insert(name, tool);
    }

    pub async fn execute_tool(&self, name: &str, arguments: serde_json::Value) -> ToolResult<ToolCallResult> {
        let tool = self.get_tool(name).ok_or_else(|| ToolError::ToolNotFound(name.to_string()))?;
        let value = tool.execute(arguments).await?;
        Ok(value)
    }

    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.tools.contains_key(tool_name)
    }

    /// Return tool execute error if tool inner error
    pub async fn execute_toolcall(&self, tool_call: &ToolCall) -> ToolResult<ToolCallResult> {
        let functioncall = &tool_call.function;
        let arguments: serde_json::Value = serde_json::from_str(&functioncall.arguments)?;
        self.execute_tool(&functioncall.name, arguments).await
    }

    pub fn to_function_defines(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|(_, tool)| tool.to_function_define()).collect()
    }

    pub fn add_tools<I: Iterator<Item = Box<dyn Tool>>>(&mut self, tools: I) {
        for tool in tools {
            self.add_tool_box(tool);
        }
    }
}
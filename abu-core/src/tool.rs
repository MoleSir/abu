use abu_api::chat::{FunctionInfo, ToolDefinition, ToolType};
use std::collections::HashMap;
use serde_json::Value;
use async_trait::async_trait;

#[async_trait]
pub trait Tool {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> serde_json::Value;
    fn to_function_define(&self) -> ToolDefinition {
        ToolDefinition {
            r#type: ToolType::Function,
            function: FunctionInfo {
                description: self.description().to_string(),
                name: self.name().to_string(),
                parameters: self.parameters()
            }
        }
    }

    async fn execute(&self, args: Value) -> ToolResult<String>;
}

pub struct ToolCollection {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolCollection {
    pub fn new<I: Iterator<Item = Box<dyn Tool>>>(tools: I) -> Self {
        Self {
            tools: tools.map(|tool| (tool.name().to_string(), tool)).collect()
        }
    }

    pub async fn execute(&self, name: &str, args: Value) -> ToolResult<String> {
        match self.get_tool(name) {
            Some(tool) => tool.execute(args).await,   
            None => Err(ToolError::ToolNotFound(name.to_string()))
        }
    }

    pub fn to_function_defines(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|(_, tool)| tool.to_function_define()).collect()
    }

    pub fn get_tool(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn add_tool(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn add_tools<I: Iterator<Item = Box<dyn Tool>>>(&mut self, tools: I) {
        for tool in tools {
            self.add_tool(tool);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("tool {0} not found")]
    ToolNotFound(String),

    #[error("arg {0} not found")]
    ArgNotFound(String),

    #[error("arg parse failed, expect: Expect {0}")]
    ArgParse(&'static str),
}

pub type ToolResult<T> = std::result::Result<T, ToolError>; 
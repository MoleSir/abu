pub mod tools;
pub mod mcp;
use std::{ffi::OsStr, path::Path};
use abu_base::chat::ToolDefinition;
use abu_mcp::McpTool;
use abu_tool::{Tool, ToolCallResult, ToolError, ToolRegister};
use mcp::McpManager;
use tracing::debug;

use crate::AgentResult;

pub struct ToolBox {
    tools: ToolRegister,
    mcp_manager: McpManager,
    tool_definitions: Vec<ToolDefinition>,
}

impl ToolBox {
    pub fn new() -> Self {
        Self {
            tools: ToolRegister::new(),
            mcp_manager: McpManager::new(),
            tool_definitions: vec![],
        }
    }

    pub async fn load_mcpconfig(&mut self, path: impl AsRef<Path>) -> AgentResult<()> {
        self.mcp_manager = McpManager::load_config(path).await?; 
        Ok(())
    }

    pub fn add_tool<T: Tool + 'static>(&mut self, tool: T) {
        debug!("add tool '{}'", tool.name());
        self.tool_definitions.push(tool.to_function_define());
        self.tools.add_tool(tool);
    } 

    pub fn add_tool_box(&mut self, tool: Box<dyn Tool>) {
        debug!("add tool '{}'", tool.name());
        self.tool_definitions.push(tool.to_function_define());
        self.tools.add_tool_box(tool);
    }

    pub async fn add_mcp_server<I, S>(&mut self, cmd: S, args: I) -> AgentResult<()> 
    where 
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        debug!("add mcp server");
        let client = self.mcp_manager.add_stdio_server(cmd, args).await?;
        for mcp_tool in client.server_tools.iter() {
            self.tool_definitions.push(mcp_tool_to_tool_defintion(mcp_tool));
        }
        Ok(())
    }

    pub async fn execute_tool(&mut self, name: &str, arguments: &str) -> AgentResult<ToolCallResult> {
        let arguments = match serde_json::from_str(arguments) {
            Err(e) => return Ok(ToolCallResult::error(e.to_string())),
            Ok(v) => v,
        };

        if self.tools.has_tool(&name) {
            let result = self.tools.execute(name, arguments).await?;
            Ok(result)
        } else if self.mcp_manager.has_tool(&name) {
            let result = self.mcp_manager.execute_toolcall(name, arguments).await?;
            Ok(result)
        } else {
            Err(ToolError::ToolNotFound(name.to_string()))?
        }
    }

    pub fn tool_definitions(&self) -> &[ToolDefinition] {
        &self.tool_definitions
    }
}


fn mcp_tool_to_tool_defintion(mcp_tool: &McpTool) -> ToolDefinition {
    ToolDefinition {
        name: mcp_tool.name.clone(),
        description: mcp_tool.description.clone().unwrap_or_default(),
        schema: serde_json::json!({
            "type": "object",
            "properties": mcp_tool.input_schema.properties.clone().unwrap_or(serde_json::json!({})),
            "required": mcp_tool.input_schema.required.clone().unwrap_or(serde_json::json!([])),
        })
    }
}
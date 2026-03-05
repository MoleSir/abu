pub mod tools;
pub mod mcp;
pub mod sandbox;
use std::{ffi::OsStr, path::{Path, PathBuf}, sync::Arc};
use abu_api::chat::{FunctionInfo, ToolCall, ToolDefinition, ToolType};
use abu_mcp::McpTool;
use abu_skill::SkillLoader;
use abu_tool::{Tool, ToolError, ToolRegister};
use mcp::McpManager;
use tools::skill::SkillTool;
use tracing::debug;

use crate::AgentResult;

pub struct AgentKit {
    tools: ToolRegister,
    mcp_manager: McpManager,
    skill_loader: Option<Arc<SkillLoader>>,
    tool_definitions: Vec<ToolDefinition>,
}

impl AgentKit {
    pub fn new() -> Self {
        Self {
            tools: ToolRegister::new(),
            skill_loader: None,
            mcp_manager: McpManager::new(),
            tool_definitions: vec![],
        }
    }

    pub async fn load_mcpconfig(&mut self, path: impl AsRef<Path>) -> AgentResult<()> {
        self.mcp_manager = McpManager::load_config(path).await?; 
        Ok(())
    }

    pub fn load_skill(&mut self, skill_dir: impl Into<PathBuf>) -> AgentResult<()> {
        let skill_loader = Arc::new(SkillLoader::load(skill_dir)?);
        // first load, add skill tool
        if self.skill_loader.is_none() {
            self.add_tool(SkillTool::new(skill_loader.clone()));
        }
        self.skill_loader = Some(skill_loader);
        Ok(())
    }

    pub fn attach_system_prompt(&self, origin: &str) -> String {
        match &self.skill_loader {
            Some(skill_loader) => format!("{}\n\n{}", origin, skill_loader.get_descriptions()),
            None => origin.to_string(),
        }
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

    pub async fn execute_tool(&mut self, tool_call: &ToolCall) -> AgentResult<String> {
        if self.tools.has_tool(&tool_call.function.name) {
            let result = self.tools.execute_toolcall(tool_call).await?;
            Ok(result.display())
        } else if self.mcp_manager.has_tool(&tool_call.function.name) {
            let result = self.mcp_manager.execute_toolcall(tool_call).await?;
            Ok(result.display())
        } else {
            Err(ToolError::ToolNotFound(tool_call.function.name.to_string()))?
        }
    }

    pub fn tool_definitions(&self) -> &[ToolDefinition] {
        &self.tool_definitions
    }
}


fn mcp_tool_to_tool_defintion(mcp_tool: &McpTool) -> ToolDefinition {
    let function = FunctionInfo {
        name: mcp_tool.name.clone(),
        description: mcp_tool.description.clone().unwrap_or_default(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": mcp_tool.input_schema.properties.clone().unwrap_or(serde_json::json!({})),
            "required": mcp_tool.input_schema.required.clone().unwrap_or(serde_json::json!([])),
        })
    };

    ToolDefinition {
        r#type: ToolType::Function,
        function
    }
}
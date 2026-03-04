use std::{ffi::OsStr, path::PathBuf, sync::Arc};

use abu_api::chat::{FunctionInfo, ToolCall, ToolDefinition, ToolType};
use abu_mcp::{client::McpClient, transport::process::McpProcessTransport, McpError, McpTool, McpToolCall, McpToolCallResult, McpToolCallResultContent};
use abu_skill::SkillLoader;
use tracing::debug;
use crate::{tool::{skill::SkillTool, Tool, ToolCallResult, ToolCollection}, AgentResult};

pub struct AgentKit {
    tools: ToolCollection,
    skill_loader: Option<Arc<SkillLoader>>,
    stdio_mcps: Vec<McpClient<McpProcessTransport>>,

    tool_definitions: Vec<ToolDefinition>,
}

impl AgentKit {
    pub fn new() -> Self {
        Self {
            tools: ToolCollection::new(),
            skill_loader: None,
            stdio_mcps: vec![],
            tool_definitions: vec![],
        }
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

    pub async fn add_mcp_server<I, S>(&mut self, cmd: S, args: I) -> Result<(), McpError> 
    where 
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        debug!("add mcp server");

        // create mcp clinet
        let transport = McpProcessTransport::new(cmd, args)?;
        let mut client = McpClient::new(transport);

        // init & get tool list 
        client.initialize().await?;
        client.tools_list().await?;

        // add tool
        for mcp_tool in client.server_tools.iter() {
            self.tool_definitions.push(mcp_tool_to_tool_defintion(mcp_tool));
        }
        self.stdio_mcps.push(client);
        Ok(())
    }

    pub async fn execute_tool(&mut self, tool_call: &ToolCall) -> AgentResult<String> {
        if self.tools.has_tool(&tool_call.function.name) {
            let result = self.tools.execute_toolcall(tool_call).await?;
            Ok(tool_call_result_string(&result))
        } else {
            for client in self.stdio_mcps.iter_mut() {
                if client.has_tool(&tool_call.function.name) {
                    let mcp_tool_call = tool_call_to_mcp_tool_call(tool_call)?;
                    let mcp_tool_call_result = client.tools_call(mcp_tool_call).await?;
                    let tool_call_result = mcp_tool_call_result.into();
                    return Ok(tool_call_result_string(&tool_call_result))
                }
            }

            Ok(format!("tool {} not found", tool_call.function.name))
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

fn tool_call_to_mcp_tool_call(tool_call: &ToolCall) -> serde_json::Result<McpToolCall> {
    Ok(McpToolCall {
        name: tool_call.function.name.clone(),
        arguments: Some(serde_json::from_str(&tool_call.function.arguments)?),
    })
}

fn tool_call_result_string(result: &ToolCallResult) -> String {
    if result.is_error {
        format!("tool execeute failed: {}", result.context)
    } else {
        format!("tool execeute success: {}", result.context)
    }
}

impl Into<ToolCallResult> for McpToolCallResult {
    fn into(self) -> ToolCallResult {
        let is_error = self.is_error.unwrap_or(false);
        let context = self
            .content
            .iter()
            .map(|content| {
                match content {
                    McpToolCallResultContent::Text { text } => text.as_str(),
                }
            })
            .collect::<Vec<&str>>()
            .join("\n");
        ToolCallResult { is_error, context }
    }
} 
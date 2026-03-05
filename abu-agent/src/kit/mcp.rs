use std::{collections::HashMap, ffi::OsStr, path::Path};
use abu_api::chat::ToolCall;
use abu_mcp::{client::McpClient, transport::process::McpProcessTransport, McpToolCall, McpToolCallResult, McpToolCallResultContent};
use abu_tool::{ToolCallResult, ToolError};
use thiserrorctx::Context;
use serde::Deserialize;
use tracing::{debug, warn};
use crate::AgentResult;

pub struct McpManager {
    pub default_protocol_version: String,
    pub stdio_servers: Vec<McpClient<McpProcessTransport>>
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpConfig {
    #[serde(default = "default_protocol_version", alias = "defaultProtocolVersion")]
    pub default_protocol_version: String,
    #[serde(alias = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    pub transport: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            default_protocol_version: default_protocol_version(),
            stdio_servers: vec![],
        }
    }

    pub async fn load_config(path: impl AsRef<Path>) -> AgentResult<Self> {
        debug!("load mcp config from {}", path.as_ref().display());
        let context = std::fs::read_to_string(path).context("read config file")?;
        let config: McpConfig = serde_json::from_str(&context).context("parse config file")?;

        let mut mcp_manager = McpManager { default_protocol_version: config.default_protocol_version, stdio_servers: vec![],};
        for (name, server_config) in config.mcp_servers {
            debug!("add mcp server {}", name);
            match server_config.transport.as_str() {
                "stdio" => {
                    mcp_manager.add_stdio_server(server_config.command, server_config.args) 
                        .await.with_context(|| format!("init client {}", name))?;
                }
                transport => warn!("unsupport transport '{}' in mcpserver {}", transport, name),
            };
        }

        Ok(mcp_manager)
    }

    pub async fn add_stdio_server<S, I>(&mut self, cmd: S, args: I) -> AgentResult<&McpClient<McpProcessTransport>> 
    where 
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let client = Self::init_stdio_clinet(cmd, args).await?;
        self.stdio_servers.push(client);
        Ok(self.stdio_servers.last().unwrap())
    }

    pub async fn execute_toolcall(&mut self, tool_call: &ToolCall) -> AgentResult<ToolCallResult> {
        for client in self.stdio_servers.iter_mut() {
            if client.has_tool(&tool_call.function.name) {
                let mcp_tool_call = tool_call_to_mcp_tool_call(tool_call)?;
                let mcp_tool_call_result = client.tools_call(mcp_tool_call).await?;
                let tool_call_result = mcp_tool_call_result_to_tool_call_result(mcp_tool_call_result);
                return Ok(tool_call_result)
            }
        }
        Err(ToolError::ToolNotFound(tool_call.function.name.to_string()))?
    }

    pub fn has_tool(&self, tool_name: &str) -> bool {
        for client in self.stdio_servers.iter() {
            if client.has_tool(tool_name) {
                return true;
            }
        }
        false
    }

    pub async fn init_stdio_clinet<I, S>(cmd: S, args: I) -> AgentResult<McpClient<McpProcessTransport>> 
    where 
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let transport = McpProcessTransport::new(cmd, args)
            .context("new process transport")?;
        let mut client = McpClient::new(transport);
        client.initialize().await.context("initialize mcpserver")?;
        client.tools_list().await.context("tools_list mcpserver")?;
        Ok(client)
    }
}

fn tool_call_to_mcp_tool_call(tool_call: &ToolCall) -> serde_json::Result<McpToolCall> {
    Ok(McpToolCall {
        name: tool_call.function.name.clone(),
        arguments: Some(serde_json::from_str(&tool_call.function.arguments)?),
    })
}

fn default_protocol_version() -> String {
    abu_mcp::LATEST_PROTOCOL_VERSION.to_string()
}

fn mcp_tool_call_result_to_tool_call_result(result: McpToolCallResult) -> ToolCallResult {
    let is_error = result.is_error.unwrap_or(false);
    let context = result
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

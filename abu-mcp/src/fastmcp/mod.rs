pub mod prelude;
use std::collections::HashMap;
use crate::transport::McpTransport;
use crate::McpResource;
use crate::{protocol::McpTool, McpClientInitializeResult, McpError, McpImplementation, McpPromptsCapability, McpResourceCapability, McpResult, McpServerCapabilities, McpServerInitializeResult, McpToolCallResult, McpToolCallResultContent, McpToolsCapability};
use crate::server::{McpServer, McpServerHandler};
use async_trait::async_trait;
use tracing::debug;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> String;
    fn to_mcptool(&self) -> McpTool;
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<Option<String>>;
}

pub trait Resource: Send + Sync {
    fn name(&self) -> String;
    fn uri(&self) -> String;
    fn description(&self) -> String;
    fn mime_type(&self) -> String;
}

pub enum Transport {
    Stdio,
}

pub struct FastMcp<T: McpTransport> {
    server: McpServer<T, FastMcpHandler>,
}

struct FastMcpHandler {
    tools: HashMap<String, Box<dyn Tool>>
}

impl FastMcpHandler {
    fn new() -> Self {
        Self { tools: HashMap::new() }
    }
}

#[async_trait]
impl McpServerHandler for FastMcpHandler {
    async fn initialize(&self, result: McpClientInitializeResult) -> McpResult<McpServerInitializeResult> 
    {
        debug!("Client connected: {} v{}", result.client_info.as_ref().map(|i| i.name.as_str()).unwrap_or(""), result.protocol_version);
        
        Ok(McpServerInitializeResult {
            protocol_version: crate::protocol::LATEST_PROTOCOL_VERSION.to_string(),
            capabilities: McpServerCapabilities {
                experimental: Some(HashMap::new()),
                logging: None,
                prompts: Some(McpPromptsCapability {
                    list_changed: Some(false),
                }),
                resources: Some(McpResourceCapability {
                    subscribe: Some(false),
                    list_changed: Some(false),
                }),
                tools: Some(McpToolsCapability {
                    list_changed: Some(false)
                })  
            },
            server_info: McpImplementation {
                name: "Hello".to_string(),
                version: "1.0.1".to_string()
            },
            instructions: None
        })
    }

    async fn tools_list(&self) -> McpResult<Vec<McpTool>> {
        Ok(self.tools.iter().map(|(_, tool)| tool.to_mcptool()).collect())
    }

    async fn resources_list(&self) -> McpResult<Vec<McpResource>> {
        Ok(vec![
            McpResource {
                uri: "./main.rs".to_string(),
                name: "main.rs".into(),
                description: Some("simple main.rs".into()),
                mime_type: "text/x-rust".into(),
            }
        ])
    }

    async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> McpResult<McpToolCallResult> {
        match self.tools.get(tool_name) {
            Some(tool) => {
                let arguments = arguments.unwrap_or(serde_json::json!({}));
                match tool.execute(arguments).await {
                    Ok(result) => {
                        let content = match result {
                            Some(ret_value) => vec![ McpToolCallResultContent::Text { text: ret_value } ],
                            None => vec! []
                        };
                        Ok(McpToolCallResult{
                            content,
                            is_error: Some(false)
                        })
                    }
                    Err(err) => Err(McpError::Other(err.to_string()))
                }

            }
            None => Err(McpError::Other(format!("No exit tool '{}'", tool_name)))
        }
    }

    async fn shutdown(&self) -> McpResult<()> {
        debug!("Server shutting down");
        Ok(())
    }
}

impl<T: McpTransport> FastMcp<T> {
    pub fn new(transport: T, tools: Vec<Box<dyn Tool>>) -> Self {
        let mut fast_handler = FastMcpHandler::new();
        for tool in tools {
            fast_handler.tools.insert(tool.name(), tool);
        }

        Self {
            server: McpServer::new(transport, fast_handler)
        }
    }

    pub async fn run(mut self) -> McpResult<()> {
        let handle = tokio::spawn(async move {
            if let Err(e) = self.server.run().await {
                eprintln!("Error in connection: {}", e);
            }
        });
        handle.await.unwrap();
        Ok(())
    }
}



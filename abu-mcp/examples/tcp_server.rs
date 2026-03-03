use std::collections::HashMap;
use tokio::net::TcpListener;
use abu_mcp::{
    server::{McpServer, McpServerHandler}, 
    transport::tcp::McpTcpTransport, 
    McpClientInitializeResult, McpImplementation, McpPromptsCapability, McpResource, McpResourceCapability, McpResult, McpServerCapabilities, McpServerInitializeResult, McpTool, McpToolCallResult, McpToolCallResultContent, McpToolInputSchema, McpToolsCapability
};
use tokio;
use tracing::{debug, info};

struct HelloHandler;

#[async_trait::async_trait]
impl McpServerHandler for HelloHandler {
    async fn initialize(&self, result: McpClientInitializeResult) -> McpResult<McpServerInitializeResult> 
    {
        info!("Client connected: v{}", result.protocol_version);
        
        Ok(McpServerInitializeResult {
            protocol_version: abu_mcp::protocol::LATEST_PROTOCOL_VERSION.to_string(),
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
        Ok(vec![
            McpTool {
                name: "get_my_name".into(),
                description: Some("Return my name to you!".into()),
                input_schema: McpToolInputSchema {
                    r#type: "object".to_string(),
                    required: None,
                    properties: None,
                }
            },
            McpTool {
                name: "say".into(),
                description: Some("print something to stdout!".into()),
                input_schema: McpToolInputSchema {
                    r#type: "object".to_string(),
                    required: Some(serde_json::json!( "value" )),
                    properties: Some(
                        serde_json::json!({
                            "value": {
                                "title": "Value",
                                "description": "the string need to print!",
                                "type": "string"
                            }
                        })
                    ),
                }
            },
        ])
    }

    async fn execute_tool(
        &self,
        tool_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> McpResult<McpToolCallResult> {
        debug!("Received method call: {} with arguments: {:?}", tool_name, arguments);
        if tool_name == "get_my_name" {
            Ok(McpToolCallResult{
                content: vec![ McpToolCallResultContent::Text { text: "molesir".to_string() } ],
                is_error: Some(false)
            })
        } else {
            todo!()
        }
    }

    async fn resources_list(&self) -> McpResult<Vec<McpResource>> {
        Ok(vec![])
    }

    async fn shutdown(&self) -> McpResult<()> {
        debug!("Shutting down");
        Ok(())
    }
}

async fn result_main() -> anyhow::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:020716").await?;
    let (stream, addr) = listener.accept().await?;
    info!("New client from {:?}", addr);

    let transport = McpTcpTransport::new(stream).await;
    let mut server = McpServer::new(transport, HelloHandler);
    let handle = tokio::spawn(async move {
        info!("Starting server");
        if let Err(e) = server.run().await {
            info!("Error in connection: {}", e);
        }
        info!("Close server");
    });
    handle.await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    if let Err(err) = result_main().await {
        eprintln!("{}", err);
    }
}


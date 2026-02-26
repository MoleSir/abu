use crate::{
    transport::{McpMessage, McpTransport}, 
    McpClientInitializeResult, 
    McpError, 
    McpErrorCode, 
    McpRequest, 
    McpResource, 
    McpResponse, 
    McpResponseError, 
    McpResult, 
    McpServerInitializeResult, 
    McpTool, 
    McpToolCall, 
    McpToolCallResult
};
use async_trait::async_trait;
use tracing::debug;

#[async_trait]
pub trait McpServerHandler: Send + Sync {
    async fn initialize(&self, result: McpClientInitializeResult) -> McpResult<McpServerInitializeResult>;

    async fn tools_list(&self) -> McpResult<Vec<McpTool>> {
        Ok(vec![])
    }

    async fn tools_call(&self, params: Option<serde_json::Value>) -> McpResult<McpToolCallResult> {
        let params = params.ok_or_else(|| {
            McpError::protocol(McpErrorCode::InvalidParams, "Missing parameters in tools/call request".to_string())
        })?;

        // Parse the parameters as CallToolParams
        let call_params: McpToolCall = serde_json::from_value(params.clone())
            .map_err(|e| McpError::protocol(McpErrorCode::InvalidParams, format!("Invalid tools/call parameters: {}", e)))?;
        
        let tool_name = &call_params.name;
        self.execute_tool(&tool_name, call_params.arguments).await
    }

    async fn resources_list(&self) -> McpResult<Vec<McpResource>> {
        Ok(vec![])
    }

    async fn execute_tool(
        &self,
        method: &str,
        arguments: Option<serde_json::Value>,
    ) -> McpResult<McpToolCallResult>;

    async fn shutdown(&self) -> McpResult<()>;
}

pub struct McpServer<T: McpTransport, H: McpServerHandler> {
    transport: T,
    handler: H,
    initilized: bool,
    shutdown: bool,
}

impl<T: McpTransport, H: McpServerHandler> McpServer<T, H> {
    pub fn new(transport: T, handler: H) -> Self {
        Self {
            transport,
            handler,
            initilized: false,
            shutdown: false,
        }
    }

    pub async fn run(&mut self) -> McpResult<()> {
        loop {
            match self.transport.receive().await? {
                McpMessage::Request(request) => {
                    let response = match self.handle_request(request.clone()).await {
                        Ok(response) => response,
                        Err(err) => McpResponse::error(request.id, McpResponseError::from(err)),
                    };
                    
                    self.transport.send(McpMessage::Response(response)).await?;
                    
                    if self.shutdown {
                        break Ok(());
                    }
                }
                McpMessage::Notification(_notification) => {}
                McpMessage::Response(_) => return Err(McpError::protocol(
                    McpErrorCode::InvalidRequest,
                    "Server received unepected response",
                ))
            }
        }
    }

    pub async fn handle_request(&mut self, request: McpRequest) -> McpResult<McpResponse> {    
        debug!("Handle a request '{}' with '{:?}'", request.method, request.params);
        match request.method.as_str() {
            "initialize" => {
                self.check_initialized(false).await?;

                let params = request.params.ok_or_else(|| {
                    McpError::protocol(McpErrorCode::InvalidParams, "Missing parameters in tools/call request".to_string())
                })?;

                let result: McpClientInitializeResult = serde_json::from_value(params)?;
                let result = self.handler.initialize(result).await?;
                
                self.initilized = true;

                Ok(McpResponse::success(request.id, Some(serde_json::to_value(result)?)))   
            }
            "shutdown" => {
                self.check_initialized(true).await?;
                self.handler.shutdown().await?;
                self.shutdown = true;
                Ok(McpResponse::success(request.id, Some(serde_json::json!({}))))
            }
            "tools/list" => {
                self.check_initialized(true).await?;
                let tools_list = self.handler.tools_list().await?;
                Ok(McpResponse::success(request.id, Some(serde_json::json!({
                    "tools": tools_list
                }))))   
            }
            "tools/call" => {
                self.check_initialized(true).await?;
                match self.handler.tools_call(request.params.clone()).await {
                    Ok(result) =>
                        Ok(McpResponse::success(request.id, Some(serde_json::to_value(result)?))),
                    Err(err) => 
                        Ok(McpResponse::error(request.id, err.into()))
                }
            }
            "resources/list" => {
                self.check_initialized(true).await?;
                let resources_list = self.handler.resources_list().await?;
                Ok(McpResponse::success(request.id, Some(serde_json::json!({
                    "resources": resources_list
                }))))
            }
            // "resources/read" => {
            //     unimplemented!()
            // }
            // "resources/templates/list" => {
            //     unimplemented!()
            // }
            // "resources/subscribe" => {
            //     unimplemented!()
            // }
            _ => {
                self.check_initialized(true).await?;
                Ok(McpResponse::success(request.id, Some(serde_json::json!({}))))
            }
        }
    }

    async fn check_initialized(&self, state: bool) -> McpResult<()> {
        if state != self.initilized {
            return Err(McpError::protocol(
                McpErrorCode::ServerNotInitialized,
                "Server not initialized",
            ));
        }

        Ok(())
    }
}

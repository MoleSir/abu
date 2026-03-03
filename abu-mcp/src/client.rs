use crate::{
    transport::{McpMessage, McpTransport}, McpClientCapabilities, McpError, McpErrorCode, McpImplementation, McpNotification, McpRequest, McpRequestId, McpResource, McpResult, McpServerCapabilities, McpTool, McpToolCall, McpToolCallResult
};
use tracing::debug;

pub struct McpClient<T: McpTransport> {
    pub implementation: McpImplementation,
    pub client_capabilities: McpClientCapabilities,
    pub server_capabilities: Option<McpServerCapabilities>,
    pub server_tools: Vec<McpTool>,
    pub server_resources: Vec<McpResource>,

    transport: T,
    request_counter: i64,
}

impl<T: McpTransport> McpClient<T> {
    pub fn new(transport: T) -> Self {
        Self {
            implementation: McpImplementation {
                name: "abu_mcp_client".to_string(),
                version: "0.1.0".to_string(),
            },
            client_capabilities: McpClientCapabilities::default(), 
            transport,
            server_capabilities: None,
            server_tools: vec![],
            server_resources: vec![],
            request_counter: 0,
        }
    }

    pub fn with_name(&mut self, name: impl Into<String>) {
        self.implementation.name = name.into();
    }

    pub fn with_version(&mut self, version: impl Into<String>) {
        self.implementation.version = version.into();
    }

    pub fn with_client_capabilities(&mut self, client_capabilities: McpClientCapabilities) {
        self.client_capabilities = client_capabilities;
    }

    pub async fn request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> McpResult<serde_json::Value> {
        debug!("Request '{}' with '{:?}'", method, params);
        let id = McpRequestId::Number(self.request_counter);
        self.request_counter += 1;

        let request = McpRequest::new(method, params, id.clone());
        self.transport.send(McpMessage::Request(request)).await?;

        // 得到 McpResponse
        match self.transport.receive().await? {
            McpMessage::Response(response) => {
                // Id 必须匹配
                if response.id == id {
                    // 判断是否接受到错误，返回 Error
                    if let Some(error) = response.error {
                        let code: McpErrorCode = error.code.into();
                        return Err(McpError::protocol(code, &error.message))
                    }
                    // 正常接受到回复信息，取出其中的 Value 返回
                    return response.result.ok_or_else(|| McpError::protocol(
                        McpErrorCode::InternalError, 
                        "Response missing result",
                    ));
                } 
            }
            _ => {}
        }
        
        Err(McpError::protocol(
            McpErrorCode::InternalError,
            "Connection closed while waiting for response",
        ))
    }

    pub async fn notify(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> McpResult<()> {
        let notification = McpNotification::new(method, params);
        self.transport.send(McpMessage::Notification(notification)).await
    }
}

impl<T: McpTransport> McpClient<T> {
    pub async fn initialize(&mut self) -> McpResult<McpServerCapabilities> {
        let params = serde_json::json!({
            "clientInfo": self.implementation,
            "capabilities": self.client_capabilities,
            "protocolVersion": crate::protocol::LATEST_PROTOCOL_VERSION,
        });

        let response = self
            .request("initialize", Some(params))
            .await?;

        let server_capabilities: McpServerCapabilities = serde_json::from_value(response)?;
        self.server_capabilities = Some(server_capabilities.clone());

        // self.notify("initialize", None).await?;

        Ok(server_capabilities)
    }

    pub async fn tools_list(&mut self) -> McpResult<&[McpTool]> {
        let response = self
            .request("tools/list", None)
            .await?;
        // Get 'tools' in response, it's a Vec<McpTool>
        let tools = response.get("tools").ok_or(McpError::Other("Except tools!".into()))?;
        let tools = serde_json::from_value(tools.clone())?;
        self.server_tools = tools;
        Ok(&self.server_tools)
    }

    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.server_tools.iter().any(|tool| tool.name == tool_name)
    }

    pub async fn tools_call(&mut self, tool_call: McpToolCall) -> McpResult<McpToolCallResult> {
        let params = serde_json::to_value(tool_call)?;
        let response = self
            .request("tools/call", Some(params))
            .await?;
        Ok(serde_json::from_value(response)?)
    }

    pub async fn shutdown(&mut self) -> McpResult<()> {
        self.request("shutdown", None).await?;
        // self.notify("exit", None).await?;
        self.transport.close().await
    }
}
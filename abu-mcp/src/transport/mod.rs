pub mod stdio;
pub mod tcp;
pub mod process;

use crate::{McpNotification, McpRequest, McpResponse, McpResult};
use serde::{Deserialize, Serialize};
use async_trait::async_trait;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpMessage {
    Request(McpRequest),
    Response(McpResponse),
    Notification(McpNotification)
}

#[async_trait]
pub trait McpTransport: Send + Sync + 'static {
    async fn send(&mut self, message: McpMessage) -> McpResult<()>;
    async fn receive(&mut self) -> McpResult<McpMessage>;
    async fn close(&mut self) -> McpResult<()>;
}
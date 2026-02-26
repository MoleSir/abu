use serde::{Serialize, Deserialize};
use derive_builder::Builder;

#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct McpNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub params: Option<serde_json::Value>,
}

impl McpNotification {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: super::JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
        }
    }
}
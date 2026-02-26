use serde::{Deserialize, Serialize};
use derive_builder::Builder;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpRequestId {
    String(String),
    Number(i64),
}

#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub params: Option<serde_json::Value>,
    pub id: McpRequestId,
}

impl McpRequest {
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>, id: McpRequestId) -> Self {
        Self {
            jsonrpc: super::JSONRPC_VERSION.to_string(),
            method: method.into(),
            params,
            id
        }
    }
}

impl fmt::Display for McpRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpRequestId::String(s) => write!(f, "{}", s),
            McpRequestId::Number(n) => write!(f, "{}", n),
        }
    }
}

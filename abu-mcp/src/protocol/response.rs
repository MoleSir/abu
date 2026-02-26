use serde::{Deserialize, Serialize};
use derive_builder::Builder;
use crate::{McpError, McpErrorCode};
use super::McpRequestId;

#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: McpRequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub error: Option<McpResponseError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct McpResponseError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub data: Option<serde_json::Value>,
}

impl McpResponse {
    pub fn success(id: McpRequestId, result: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: super::JSONRPC_VERSION.to_string(),
            id,
            result,
            error: None
        }
    }

    pub fn error(id: McpRequestId, error: McpResponseError) -> Self {
        Self {
            jsonrpc: super::JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error)
        }
    }
}

impl From<McpError> for McpResponseError {
    fn from(err: McpError) -> Self {
        match err {
            McpError::Protocol {
                code,
                message,
                data,
            } => McpResponseError {
                code: code.into(),
                message,
                data,
            },
            McpError::Transport(msg) => McpResponseError {
                code: McpErrorCode::InternalError.into(),
                message: format!("Transport McpError: {}", msg),
                data: None,
            },
            McpError::Serialization(err) => McpResponseError {
                code: McpErrorCode::ParseError.into(),
                message: err.to_string(),
                data: None,
            },
            McpError::Io(err) => McpResponseError {
                code: McpErrorCode::InternalError.into(),
                message: err.to_string(),
                data: None,
            },
            McpError::Other(msg) => McpResponseError {
                code: McpErrorCode::InternalError.into(),
                message: msg,
                data: None,
            },
        }
    }
}
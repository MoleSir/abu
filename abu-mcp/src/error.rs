use std::fmt;
use thiserror::Error;

pub type McpResult<R> = Result<R, McpError>; 

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum McpErrorCode {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
    ServerNotInitialized = -32002,
    UnknownErrorCode = -32001,
    RequestFailed = -32000,
}

/// Main error type for MCP operations
#[derive(Error, Debug, Clone)]
pub enum McpError {
    #[error("MCP protocol error: {code:?} - {message}")]
    Protocol {
        code: McpErrorCode,
        message: String,
        data: Option<serde_json::Value>,
    },

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<i32> for McpErrorCode {
    fn from(code: i32) -> Self {
        match code {
            -32700 => McpErrorCode::ParseError,
            -32600 => McpErrorCode::InvalidRequest,
            -32601 => McpErrorCode::MethodNotFound,
            -32602 => McpErrorCode::InvalidParams,
            -32603 => McpErrorCode::InternalError,
            -32002 => McpErrorCode::ServerNotInitialized,
            -32001 => McpErrorCode::UnknownErrorCode,
            -32000 => McpErrorCode::RequestFailed,
            _ => McpErrorCode::UnknownErrorCode,
        }
    }
}

impl From<McpErrorCode> for i32 {
    fn from(code: McpErrorCode) -> Self {
        code as i32
    }
}

impl fmt::Display for McpErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}


impl McpError {
    pub fn protocol(code: McpErrorCode, message: impl Into<String>) -> Self {
        McpError::Protocol {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(self, data: serde_json::Value) -> Self {
        match self {
            McpError::Protocol {
                code,
                message,
                data: _,
            } => McpError::Protocol {
                code,
                message,
                data: Some(data),
            },
            _ => self,
        }
    }
}

impl From<std::io::Error> for McpError {
    fn from(err: std::io::Error) -> Self {
        McpError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for McpError {
    fn from(err: serde_json::Error) -> Self {
        McpError::Serialization(err.to_string())
    }
}

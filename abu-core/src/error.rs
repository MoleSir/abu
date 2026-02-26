use abu_api::{chat::ChatRequestBuilderError, ApiError};

use crate::tool::ToolError;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),
    
    #[error(transparent)]
    Tool(#[from] ToolError),

    #[error("Unsupport tool {0}")]
    UnsupportTool(String),

    #[error(transparent)]
    Api(#[from] ApiError),

    #[error(transparent)]
    EnvVar(#[from] std::env::VarError),

    #[error(transparent)]
    Dotenv(#[from] dotenv::Error),
    
    #[error("Except messgae {0}")]
    ExceptMessage(&'static str),

    #[error(transparent)]
    ChatRequest(#[from] ChatRequestBuilderError),

    #[error("no choise in response")]
    NoChoise,
}

pub type CoreResult<T> = std::result::Result<T, CoreError>;
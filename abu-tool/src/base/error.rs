#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error("tool {0} not found")]
    ToolNotFound(String),

    #[error("arg {0} not found")]
    ArgNotFound(String),

    #[error("arg parse failed, expect: Expect {0}")]
    ArgParse(&'static str),
}

pub type ToolResult<T> = std::result::Result<T, ToolError>; 
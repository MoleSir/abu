use abu_api::{chat::ChatRequestBuilderError, ApiError};
use abu_mcp::McpError;
use abu_skill::SkillError;
use abu_tool::ToolError;

// #[derive(Debug, thiserror::Error)]
#[thiserrorctx::context_error]
pub enum AgentError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Skill(#[from] SkillError),

    #[error(transparent)]
    Memory(anyhow::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    Mcp(#[from] McpError),
    
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

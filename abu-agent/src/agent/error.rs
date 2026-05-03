use abu_base::chat::ChatRequestBuilderError;
use abu_mcp::McpError;
use abu_skill::SkillError;
use abu_tool::ToolError;
use crate::model::ChatModelError;

#[thiserrorctx::context_error]
pub enum AgentError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Chat(#[from] ChatModelError),

    #[error(transparent)]
    Skill(#[from] SkillError),

    #[error("memory error: {0}")]
    Memory(String),

    #[error("hook error: {0}")]
    Hook(String),

    #[error("compact error: {0}")]
    Compact(String),

    #[error("when {0} error {1}")]
    Middleware(&'static str, String),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    Mcp(#[from] McpError),
    
    #[error(transparent)]
    Tool(#[from] ToolError),

    #[error("Unsupport tool {0}")]
    UnsupportTool(String),

    #[error("chat provide error: {0}")]
    ChatProvider(String),

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

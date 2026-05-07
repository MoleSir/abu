mod error;
use abu_base::chat::ChatMessage;
pub use error::*;
mod build;
pub use build::*;
mod lloop;
pub use lloop::*;
mod context;
pub use context::*;

use abu_provider::ChatProvide;
use crate::hook::HookManager;
use crate::memory::{Memory, NoMemory};
use crate::middleware::MiddlewareManager;
use crate::model::ChatModel;
use crate::tool::ToolManager;
use crate::compact::{ContextCompact, NoContextCompact};

#[derive(Clone)]
pub struct AgentConfig {
    pub max_iteration: usize,
    pub temperature: f64,
}

pub struct Agent<P: ChatProvide, M: Memory = NoMemory, C: ContextCompact = NoContextCompact> {
    pub config: AgentConfig,
    pub system_prompt: String,
    pub session: Vec<ChatMessage>,

    pub llm: ChatModel<P>,
    pub memory: M,
    pub compact: C,
    pub tools: ToolManager,
    pub hooks: HookManager,
    pub middlewares: MiddlewareManager,
}

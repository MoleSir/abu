mod error;
pub use error::*;
mod build;
pub use build::*;
mod lloop;
pub use lloop::*;

use abu_provider::ChatProvide;
use crate::context::ContextBuilder;
use crate::hook::HookManager;
use crate::memory::Memory;
use crate::middleware::MiddlewareManager;
use crate::model::ChatModel;
use crate::tool::ToolManager;

#[derive(Clone)]
pub struct AgentConfig {
    pub max_iteration: usize,
    pub temperature: f64,
}

pub struct Agent<P: ChatProvide, M: Memory> {
    pub config: AgentConfig,
    pub llm: ChatModel<P>,
    pub memory: M,
    pub context_builder: ContextBuilder,
    pub tools: ToolManager,
    pub hooks: HookManager,
    pub middlewares: MiddlewareManager,
}

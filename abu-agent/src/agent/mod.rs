mod error;
pub use error::*;
mod build;
pub use build::*;
mod lloop;

use abu_provider::ChatProvide;
use crate::context::ContextBuilder;
use crate::hook::HookManager;
use crate::memory::Memory;
use crate::middleware::MiddlewareManager;
use crate::model::ChatModel;
use crate::toolbox::ToolBox;

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
    pub toolbox: ToolBox,
    pub hooks: HookManager,
    pub middlewares: MiddlewareManager,
}

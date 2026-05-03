use std::sync::Arc;
use abu_provider::ChatProvide;
use tokio::sync::RwLock;
use crate::{compact::ContextCompact, memory::Memory, Agent, AgentControl, AgentError, AgentResult};

pub struct SubAgentTool<P: ChatProvide, M: Memory, C: ContextCompact> {
    pub name: String,
    pub description: String,
    pub agent: Arc<RwLock<Agent<P, M, C>>>, 
}

impl<P: ChatProvide, M: Memory, C: ContextCompact> SubAgentTool<P, M, C> {
    pub fn new(agent: Agent<P, M, C>, name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            agent: Arc::new(RwLock::new(agent)),
        }
    }
}

#[abu_tool::tool(
    struct_name = SubAgentTool,
    generics = "P: ChatProvide, M: Memory, C: ContextCompact", 
    name = self.name.clone(), 
    description = self.description.clone(), 
)]
pub async fn run(&self, query: &str) -> AgentResult<AgentControl<String>> {
    let mut agent = self.agent.write().await;
    agent.memory.clear().await.map_err(|e| AgentError::Memory(e.to_string()))?;
    agent.run(query).await
}

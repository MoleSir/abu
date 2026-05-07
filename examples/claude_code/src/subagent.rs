use abu_agent::{
    compact::NoContextCompact,
    memory::NoMemory,
    model::ChatModel,
    tool::SubAgentTool,
    AgentBuilder,
};
use abu_provider::deepseek::DeepSeek;

use crate::{tools::{Bash, ReadFile, WriteFile}, hook::SilentHook};

pub async fn build_subagent(
) -> anyhow::Result<SubAgentTool<DeepSeek, NoMemory, NoContextCompact>> {
    let model = ChatModel::deepseek("deepseek-chat")?;
    let subagent = AgentBuilder::new(model)
        .system_prompt(
            "You are a coding subagent. Complete the given task thoroughly, \
             then summarize your findings concisely. You share the filesystem \
             but have a fresh conversation context.",
        )
        .with_hook(SilentHook::new())
        .with_tool(Bash::new())
        .with_tool(ReadFile::new())
        .with_tool(WriteFile::new())
        .build()
        .await?;

    Ok(SubAgentTool::new(
        subagent,
        "task",
        "Delegate a task to a subagent with fresh context. \
         The subagent shares the filesystem but not conversation history. \
         Use for exploration, research, or isolated subtasks.",
    ))
}

use abu_agent::{
    compact::NoContextCompact,
    memory::NoMemory,
    model::ChatModel,
    tool::SubAgentTool,
    AgentBuilder,
};
use abu_provider::ChatProvide;

use crate::{
    ui::hook::SilentHook,
    tools::{Bash, EditFile, Glob, Grep, ReadFile, WriteFile},
};

// ============================================================================
// Task subagent — general purpose, can read/write/execute
// ============================================================================

pub async fn build_task_subagent<P: ChatProvide + 'static>(
    model: ChatModel<P>,
) -> anyhow::Result<SubAgentTool<P, NoMemory, NoContextCompact>> {
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
        .with_tool(EditFile::new())
        .with_tool(Glob::new())
        .with_tool(Grep::new())
        .build()
        .await?;

    Ok(SubAgentTool::new(
        subagent,
        "task",
        "Delegate a general-purpose task to a subagent with fresh context. \
         Use for implementing code, fixing bugs, or any non-trivial work that \
         benefits from isolated context. The subagent can read, write, edit, \
         and execute commands.",
    ))
}

// ============================================================================
// Explore subagent — read-only, for code exploration and research
// ============================================================================

pub async fn build_explore_subagent<P: ChatProvide + 'static>(
    model: ChatModel<P>,
) -> anyhow::Result<SubAgentTool<P, NoMemory, NoContextCompact>> {
    let subagent = AgentBuilder::new(model)
        .system_prompt(
            "You are an exploration subagent. Your job is to thoroughly search \
             and understand code. Read files, search for patterns with Grep, \
             find files with Glob, and report your findings clearly.\n\n\
             IMPORTANT: You are READ-ONLY. Do NOT write or edit any files. \
             Do NOT run bash commands that modify the filesystem. \
             Only use Bash for read-only operations like git log, ls, cat, etc.\n\n\
             Report: what you found, where it is (file:line), and how it connects \
             to the broader codebase.",
        )
        .with_hook(SilentHook::new())
        .with_tool(ReadFile::new())
        .with_tool(Glob::new())
        .with_tool(Grep::new())
        .with_tool(Bash::new())
        .build()
        .await?;

    Ok(SubAgentTool::new(
        subagent,
        "explore",
        "Launch an exploration subagent to search and understand code. \
         The explore agent is read-only — it can read files, search with Grep, \
         find files with Glob, and run read-only bash commands. \
         Use for: finding where something is defined, understanding how \
         components connect, searching for patterns across the codebase. \
         The agent returns a structured report of its findings.",
    ))
}

// ============================================================================
// Plan subagent — read-only, for architecture and design planning
// ============================================================================

pub async fn build_plan_subagent<P: ChatProvide + 'static>(
    model: ChatModel<P>,
) -> anyhow::Result<SubAgentTool<P, NoMemory, NoContextCompact>> {
    let subagent = AgentBuilder::new(model)
        .system_prompt(
            "You are a software architect subagent. Your job is to design \
             implementation plans for coding tasks.\n\n\
             When given a task:\n\
             1. Explore the relevant code to understand existing patterns\n\
             2. Identify which files need to change\n\
             3. Design a step-by-step implementation approach\n\
             4. Consider edge cases, error handling, and testing\n\
             5. Report your plan with specific file paths and changes\n\n\
             IMPORTANT: You are READ-ONLY. Design the plan, don't implement it. \
             Think about architectural trade-offs and mention them.",
        )
        .with_hook(SilentHook::new())
        .with_tool(ReadFile::new())
        .with_tool(Glob::new())
        .with_tool(Grep::new())
        .with_tool(Bash::new())
        .build()
        .await?;

    Ok(SubAgentTool::new(
        subagent,
        "plan",
        "Launch a planning subagent to design an implementation approach. \
         The plan agent is read-only — it explores the codebase, identifies \
         what needs to change, and produces a detailed step-by-step plan with \
         specific file paths and architectural considerations. \
         Use before starting a complex implementation to avoid wasted work.",
    ))
}

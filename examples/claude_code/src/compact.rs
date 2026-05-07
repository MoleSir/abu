use std::path::PathBuf;

use abu_agent::{
    compact::ContextCompact,
    middleware::{MiddlewareFlow, ToolResultMiddleware},
    model::ChatModel,
    AgentContext,
};
use abu_provider::{chat::ChatMessage, ChatProvide};
use abu_tool::ToolCallResult;
use tokio::fs;

const KEEP_RECENT_TOOL_RESULTS: usize = 3;

// ============================================================================
// SummarizationCompact
// ============================================================================

pub struct SummarizationCompact<P> {
    llm: ChatModel<P>,
    summary_threshold: usize,
}

impl<P: ChatProvide> SummarizationCompact<P> {
    pub fn new(llm: ChatModel<P>, summary_threshold: usize) -> Self {
        Self {
            llm,
            summary_threshold,
        }
    }

    fn format_message(msg: &ChatMessage) -> String {
        format!("{}: {}", msg.role(), msg.content())
    }

    fn micro_compact(&mut self, session: &mut Vec<ChatMessage>) -> anyhow::Result<()> {
        let mut tool_msg_indices: Vec<usize> = vec![];
        for (i, msg) in session.iter().enumerate() {
            if let ChatMessage::Tool(_) = msg {
                tool_msg_indices.push(i);
            }
        }

        if tool_msg_indices.len() <= KEEP_RECENT_TOOL_RESULTS {
            return Ok(());
        }

        for i in tool_msg_indices
            .into_iter()
            .rev()
            .skip(KEEP_RECENT_TOOL_RESULTS)
            .rev()
        {
            let msg = &mut session[i];
            if let ChatMessage::Tool(msg) = msg {
                if msg.content.len() > 120 {
                    msg.content = "[Earlier tool result compacted. Re-run the tool if you need full detail.]".to_string();
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl<P: ChatProvide> ContextCompact for SummarizationCompact<P> {
    type Error = anyhow::Error;

    async fn compact(&mut self, context: &mut AgentContext) -> Result<(), Self::Error> {
        self.micro_compact(&mut context.session)?;

        if context.session.len() + context.memory.len() + 1 <= self.summary_threshold {
            return Ok(());
        }

        let buffer_text = context
            .session
            .iter()
            .map(|m| Self::format_message(m))
            .collect::<Vec<_>>()
            .join("\n");

        let summarization_prompt = format!(
            "Summarize this conversation for continuity. Include: \
             1) What was accomplished, 2) Current state, 3) Key decisions made. \
             Be concise but preserve critical details.\n\n{}",
            buffer_text
        );

        let messages = vec![
            ChatMessage::system("You are an expert summarization engine."),
            ChatMessage::user(summarization_prompt),
        ];

        match self.llm.chat(messages).await {
            Ok(response) => {
                let mut session = vec![];
                session.push(ChatMessage::user(format!(
                    "[Conversation compressed]: {}",
                    response.message.content
                )));
                session.push(ChatMessage::assistant(
                    "Understood. I have the context from the summary. Continuing.",
                    [],
                ));
                context.session = session;
            }
            Err(_) => {
                // If summarization fails, keep the oldest messages and trim
                let keep = context.session.len().saturating_sub(self.summary_threshold);
                context.session = context.session.split_off(keep);
            }
        }

        Ok(())
    }
}

// ============================================================================
// CompactToolResult
// ============================================================================

pub struct CompactToolResult {
    pub cache_dir: PathBuf,
}

impl CompactToolResult {
    pub fn new<P: Into<PathBuf>>(cache_dir: P) -> anyhow::Result<Self> {
        let cache_dir = cache_dir.into();
        std::fs::create_dir_all(&cache_dir)?;
        Ok(Self { cache_dir })
    }
}

const PERSIST_THRESHOLD: usize = 3000;
const PREVIEW_CHARS: usize = 2000;

#[async_trait::async_trait]
impl ToolResultMiddleware for CompactToolResult {
    type Error = anyhow::Error;

    async fn intercept(
        &mut self,
        tool_call: &abu_provider::chat::ToolCall,
        result: &mut ToolCallResult,
    ) -> Result<MiddlewareFlow, Self::Error> {
        if result.context.len() <= PERSIST_THRESHOLD {
            return Ok(MiddlewareFlow::Continue);
        }

        let stored_path = self.cache_dir.join(format!("{}.txt", tool_call.id));
        fs::write(&stored_path, &result.context).await?;

        let preview: String = result.context.chars().take(PREVIEW_CHARS).collect();
        let absolute_path = std::fs::canonicalize(&stored_path)?;

        result.context = format!(
            "<persisted-output>\nFull output saved to: {:?}\nPreview:\n{}\n</persisted-output>",
            absolute_path, preview
        );

        Ok(MiddlewareFlow::Continue)
    }
}

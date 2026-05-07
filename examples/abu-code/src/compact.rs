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

// ============================================================================
// IncrementalCompact
//
// Instead of re-summarizing the entire conversation every time, this only
// summarizes NEW messages since the last compaction, then merges with the
// existing summary. Uses character count as a proxy for token count.
// ============================================================================

/// Trigger compaction when unsummarized messages exceed ~2000 tokens worth of chars
const COMPACT_TRIGGER_CHARS: usize = 8000;
/// Only keep the N most recent tool results in full
const KEEP_RECENT_TOOL_RESULTS: usize = 3;
/// Prefix for summary messages embedded in the session
const SUMMARY_MARKER: &str = "[Conversation history summary]: ";

pub struct IncrementalCompact<P> {
    llm: ChatModel<P>,
    /// Accumulated summary of already-compacted messages
    summary: String,
    /// How many messages from the session have been compacted into `summary`
    compacted_count: usize,
}

impl<P: ChatProvide> IncrementalCompact<P> {
    pub fn new(llm: ChatModel<P>) -> Self {
        Self {
            llm,
            summary: String::new(),
            compacted_count: 0,
        }
    }

    /// Scan a loaded session for summary markers and restore compact state.
    /// This avoids re-summarizing already-compacted messages on resume.
    pub fn restore_state(&mut self, session: &[ChatMessage]) {
        for (i, msg) in session.iter().enumerate() {
            if let ChatMessage::User(user) = msg {
                if let Some(summary) = user.content.strip_prefix(SUMMARY_MARKER) {
                    self.summary = summary.to_string();
                    // +2: the summary user msg + the assistant acknowledgment that follows
                    self.compacted_count = i + 2;
                    return;
                }
            }
        }
        // No summary markers found — start fresh
        self.summary.clear();
        self.compacted_count = 0;
    }

    /// Replace old tool results with a placeholder, keeping only recent ones.
    fn micro_compact(&self, session: &mut [ChatMessage]) {
        let mut tool_indices: Vec<usize> = vec![];
        for (i, msg) in session.iter().enumerate() {
            if let ChatMessage::Tool(_) = msg {
                tool_indices.push(i);
            }
        }

        if tool_indices.len() <= KEEP_RECENT_TOOL_RESULTS {
            return;
        }

        for i in tool_indices
            .into_iter()
            .rev()
            .skip(KEEP_RECENT_TOOL_RESULTS)
        {
            if let ChatMessage::Tool(msg) = &mut session[i] {
                if msg.content.len() > 120 {
                    msg.content =
                        "[Earlier tool result compacted. Re-run the tool if needed.]"
                            .to_string();
                }
            }
        }
    }

    /// Count total characters in a slice of messages.
    fn char_count(messages: &[ChatMessage]) -> usize {
        messages.iter().map(|m| m.content().chars().count()).sum()
    }
}

#[async_trait::async_trait]
impl<P: ChatProvide> ContextCompact for IncrementalCompact<P> {
    type Error = anyhow::Error;

    async fn compact(&mut self, context: &mut AgentContext) -> Result<(), Self::Error> {
        // Always run micro-compaction on tool results
        self.micro_compact(&mut context.session);

        // Only the unsummarized (new) portion
        let new_messages = &context.session[self.compacted_count..];
        let new_char_count = Self::char_count(new_messages);

        if new_char_count < COMPACT_TRIGGER_CHARS {
            return Ok(());
        }

        // Format only the NEW messages for summarization
        let new_text: String = new_messages
            .iter()
            .map(|m| format!("{}: {}", m.role(), m.content()))
            .collect::<Vec<_>>()
            .join("\n");

        // Build summarization prompt — merge with existing summary if any
        let summarization_prompt = if self.summary.is_empty() {
            format!(
                "Summarize this conversation for continuity. Include: \
                 1) What was accomplished, 2) Current state, 3) Key decisions made. \
                 Be concise but preserve critical details.\n\n{}",
                new_text
            )
        } else {
            format!(
                "Existing summary of earlier conversation:\n{}\n\n\
                 New messages to incorporate:\n{}\n\n\
                 Merge the new information into the existing summary. \
                 Keep the same format: 1) Accomplished, 2) Current state, 3) Key decisions.",
                self.summary, new_text
            )
        };

        let messages = vec![
            ChatMessage::system("You are an expert summarization engine. Output only the summary, no preamble."),
            ChatMessage::user(summarization_prompt),
        ];

        match self.llm.chat(messages).await {
            Ok(response) => {
                self.summary = response.message.content.trim().to_string();
                self.compacted_count = context.session.len();

                // Replace session with summary + recent messages (keep last few for continuity)
                let keep_recent = 4usize; // Keep the last few exchanges for immediate context
                let split_at = context.session.len().saturating_sub(keep_recent);

                let recent = context.session.split_off(split_at);
                let mut compacted = vec![
                    ChatMessage::user(format!(
                        "{}{}",
                        SUMMARY_MARKER, self.summary
                    )),
                    ChatMessage::assistant(
                        "Understood. I have the full context from the summary.",
                        [],
                    ),
                ];
                compacted.extend(recent);
                context.session = compacted;

                // Reset counter: the 2 summary messages + kept recent messages
                self.compacted_count = 2;
            }
            Err(_) => {
                // Fallback: drop oldest messages
                let excess = context
                    .session
                    .len()
                    .saturating_sub(COMPACT_TRIGGER_CHARS / 80); // rough msg estimate
                if excess > 0 {
                    context.session = context.session.split_off(excess);
                }
                self.compacted_count = 0;
                self.summary.clear();
            }
        }

        Ok(())
    }
}

// ============================================================================
// CompactToolResult — persist large tool outputs to disk
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

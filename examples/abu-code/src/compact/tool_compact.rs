use std::path::PathBuf;
use abu_agent::middleware::{MiddlewareFlow, ToolResultMiddleware};
use abu_tool::ToolCallResult;

pub struct CompactToolResult {
    cache_dir: PathBuf,
}

impl CompactToolResult {
    pub async fn new(cache_dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let cache_dir = cache_dir.into();
        tokio::fs::create_dir_all(&cache_dir).await?;
        Ok(Self { cache_dir: cache_dir.into() })
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
        tokio::fs::write(&stored_path, &result.context).await?;

        let preview: String = result.context.chars().take(PREVIEW_CHARS).collect();
        let absolute_path = tokio::fs::canonicalize(&stored_path).await?;

        result.context = format!(
            "<persisted-output>\nFull output saved to: {:?}\nPreview:\n{}\n</persisted-output>",
            absolute_path, preview
        );

        Ok(MiddlewareFlow::Continue)
    }
}
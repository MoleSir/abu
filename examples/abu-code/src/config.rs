use abu_agent::model::ChatModel;

/// Create a ChatModel from environment variables.
///
///   CHAT_MODEL       — model name (default: "deepseek-chat")
///   DEEPSEEK_BASE_URL — API endpoint (default: https://api.deepseek.com)
///   DEEPSEEK_API_KEY  — API key
///
/// Set DEEPSEEK_BASE_URL to use any OpenAI-compatible provider.
pub fn create_chat_model() -> anyhow::Result<ChatModel<impl abu_provider::ChatProvide>> {
    let model_name = std::env::var("CHAT_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());
    Ok(ChatModel::deepseek(&model_name)?)
}

/// Create a second ChatModel for compaction/summarization.
/// Uses CHAT_MODEL_COMPACT if set, otherwise falls back to CHAT_MODEL.
pub fn create_compact_model() -> anyhow::Result<ChatModel<impl abu_provider::ChatProvide>> {
    let model_name = std::env::var("CHAT_MODEL_COMPACT")
        .or_else(|_| std::env::var("CHAT_MODEL"))
        .unwrap_or_else(|_| "deepseek-chat".to_string());
    Ok(ChatModel::deepseek(&model_name)?)
}

/// Get the configured model name for display.
pub fn model_name() -> String {
    std::env::var("CHAT_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string())
}


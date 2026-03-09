use super::message::*;
use super::tool::*;
use derive_builder::Builder;
use serde::Serialize;
use strum::{Display, EnumString, EnumVariantNames};

#[derive(Debug, Clone, Builder)]
#[builder(setter(into, strip_option))]
pub struct ChatRequest {
    /// ID of the model to use. See the model endpoint compatibility table for details on which models work with the Chat API.
    pub model: String,

    /// A list of messages comprising the conversation so far.
    #[builder(default)]
    pub messages: Vec<ChatMessage>,

    /// A list of tools the model may call. Currently, only functions are supported as a tool. Use this to provide a list of functions the model may generate JSON inputs for.
    #[builder(default)]
    pub tools: Vec<ToolDefinition>,

    /// If set, partial message deltas will be sent, like in ChatGPT. Tokens will be sent as data-only server-sent events as they become available, with the stream terminated by a data: [DONE] message.
    #[builder(default)]
    pub stream: Option<bool>,   

    /// What sampling temperature to use, between 0 and 2. Higher values like 0.8 will make the output more random, while lower values like 0.2 will make it more focused and deterministic. We generally recommend altering this or top_p but not both.
    #[builder(default)]
    pub temperature: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatResponseFormatObject {
    r#type: ChatResponseFormat,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, EnumString, Display, EnumVariantNames)]
#[serde(rename_all = "snake_case")]
pub enum ChatResponseFormat {
    Text,
    #[default]
    Json,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: impl Into<Vec<ChatMessage>>) -> Self {
        ChatRequestBuilder::default()
            .model(model.into())
            .messages(messages)
            .build()
            .unwrap()
    }

    pub fn new_with_tools(
        model: impl Into<String>,
        messages: impl Into<Vec<ChatMessage>>,
        tools: impl Into<Vec<ToolDefinition>>,
    ) -> Self {
        ChatRequestBuilder::default()
            .model(model.into())
            .messages(messages)
            .tools(tools)
            .build()
            .unwrap()
    }
}

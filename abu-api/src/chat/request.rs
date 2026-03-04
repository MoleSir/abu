use crate::ApiRequest;
use super::message::*;
use super::tool::*;
use super::ChatResponse;
use derive_builder::Builder;
use serde::Serialize;
use strum::{Display, EnumString, EnumVariantNames};

#[derive(Debug, Clone, Builder, Serialize)]
#[builder(setter(into, strip_option))]
pub struct ChatRequestRef<'a> {
    /// A list of messages comprising the conversation so far.
    #[builder(default)]
    pub messages: &'a [ChatMessage],

    /// ID of the model to use. See the model endpoint compatibility table for details on which models work with the Chat API.
    pub model: String,

    /// If set, partial message deltas will be sent, like in ChatGPT. Tokens will be sent as data-only server-sent events as they become available, with the stream terminated by a data: [DONE] message.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,   

    /// What sampling temperature to use, between 0 and 2. Higher values like 0.8 will make the output more random, while lower values like 0.2 will make it more focused and deterministic. We generally recommend altering this or top_p but not both.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// A list of tools the model may call. Currently, only functions are supported as a tool. Use this to provide a list of functions the model may generate JSON inputs for.
    #[builder(default)]
    #[serde(skip_serializing_if = "is_empty_slice")]
    pub tools: &'a [ToolDefinition],
}

fn is_empty_slice<T>(t: &&[T]) -> bool {
    t.is_empty()
}

#[derive(Debug, Clone, Builder, Serialize)]
#[builder(setter(into, strip_option))]
pub struct ChatRequest {
    /// A list of messages comprising the conversation so far.
    #[builder(default)]
    pub messages: Vec<ChatMessage>,

    /// ID of the model to use. See the model endpoint compatibility table for details on which models work with the Chat API.
    pub model: String,

    /// Number between -2.0 and 2.0. Positive values penalize new tokens based on their existing frequency in the text so far, decreasing the model's likelihood to repeat the same line verbatim.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// The maximum number of tokens to generate in the chat completion.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,

    /// How many chat completion choices to generate for each input message. Note that you will be charged based on the number of generated tokens across all of the choices. Keep n as 1 to minimize costs.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<usize>,  

    /// Number between -2.0 and 2.0. Positive values penalize new tokens based on whether they appear in the text so far, increasing the model's likelihood to talk about new topics.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// An object specifying the format that the model must output. Setting to { "type": "json_object" } enables JSON mode, which guarantees the message the model generates is valid JSON.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ChatResponseFormatObject>,

    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<usize>,

    /// Up to 4 sequences where the API will stop generating further tokens.
    // TODO: make this as an enum
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<String>,

    /// If set, partial message deltas will be sent, like in ChatGPT. Tokens will be sent as data-only server-sent events as they become available, with the stream terminated by a data: [DONE] message.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,   

    /// What sampling temperature to use, between 0 and 2. Higher values like 0.8 will make the output more random, while lower values like 0.2 will make it more focused and deterministic. We generally recommend altering this or top_p but not both.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// An alternative to sampling with temperature, called nucleus sampling, where the model considers the results of the tokens with top_p probability mass. So 0.1 means only the tokens comprising the top 10% probability mass are considered. We generally recommend altering this or temperature but not both.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// A list of tools the model may call. Currently, only functions are supported as a tool. Use this to provide a list of functions the model may generate JSON inputs for.
    #[builder(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,

    /// Controls which (if any) function is called by the model. none means the model will not call a function and instead generates a message. auto means the model can pick between generating a message or calling a function. Specifying a particular function via {"type: "function", "function": {"name": "my_function"}} forces the model to call that function. none is the default when no functions are present. auto is the default if functions are present.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// A unique identifier representing your end-user, which can help OpenAI to monitor and detect abuse.
    #[builder(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
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

impl ApiRequest for ChatRequest {
    type Response = ChatResponse;
    fn url(base_url: &str) -> String {
        format!("{}/chat/completions", base_url)
    }
}

impl<'a> ApiRequest for ChatRequestRef<'a> {
    type Response = ChatResponse;
    fn url(base_url: &str) -> String {
        format!("{}/chat/completions", base_url)
    }
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

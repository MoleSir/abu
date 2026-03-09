use serde::{Deserialize, Serialize};
use abu_base::{chat::{ChatMessage, ChatRequest}, common::Role};

// =========================================================================== //
//                              Anthropic Request DTOs
// =========================================================================== //

#[derive(Serialize)]
pub struct AnthropicChatRequestDTO<'a> {
    pub model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub messages: Vec<AnthropicMessageDTO<'a>>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<AnthropicToolDTO<'a>>,
}

#[derive(Serialize)]
pub struct AnthropicMessageDTO<'a> {
    pub role: &'static str, // "user" / "assistant"
    pub content: Vec<AnthropicRequestContentBlockDTO<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicRequestContentBlockDTO<'a> {
    Text { 
        text: &'a str 
    },
    ToolUse { 
        id: &'a str, 
        name: &'a str, 
        input: &'a str,
    },
    ToolResult { 
        tool_use_id: &'a str, 
        content: &'a str, 
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool> 
    },
}

#[derive(Serialize)]
pub struct AnthropicToolDTO<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub input_schema: &'a serde_json::Value,
}

impl<'a> AnthropicChatRequestDTO<'a> {
    pub fn from_request(req: &'a ChatRequest) -> Self {
        let mut system_prompts = Vec::new();
        let mut messages_dto = Vec::new();

        for m in &req.messages {
            match m {
                ChatMessage::System(msg) => {
                    system_prompts.push(msg.content.clone());
                }
                ChatMessage::User(msg) => {
                    messages_dto.push(AnthropicMessageDTO {
                        role: "user",
                        content: vec![AnthropicRequestContentBlockDTO::Text {
                            text: &msg.content,
                        }],
                    });
                }
                ChatMessage::Assistant(msg) => {
                    let mut content_blocks = Vec::new();
                    
                    if !msg.content.is_empty() {
                        content_blocks.push(AnthropicRequestContentBlockDTO::Text {
                            text: &msg.content
                        });
                    }

                    for tc in &msg.tool_calls {
                        content_blocks.push(AnthropicRequestContentBlockDTO::ToolUse {
                            id: &tc.id,
                            name: &tc.name,
                            input: &tc.arguments,
                        });
                    }

                    messages_dto.push(AnthropicMessageDTO {
                        role: "assistant",
                        content: content_blocks,
                    });
                }
                ChatMessage::Tool(msg) => {
                    messages_dto.push(AnthropicMessageDTO {
                        role: "user",
                        content: vec![AnthropicRequestContentBlockDTO::ToolResult {
                            tool_use_id: &msg.tool_call_id,
                            content: &msg.content,
                            is_error: None,
                        }],
                    });
                }
            }
        }

        let system = if system_prompts.is_empty() {
            None
        } else {
            Some(system_prompts.join("\n"))
        };

        let tools_dto = req.tools.iter().map(|t| AnthropicToolDTO {
            name: &t.name,
            description: &t.description,
            input_schema: &t.schema 
        }).collect();

        Self {
            model: &req.model,
            system,
            messages: messages_dto,
            max_tokens: 4096,
            temperature: req.temperature,
            tools: tools_dto,
        }
    }
}

// =========================================================================== //
//                              Anthropic Response DTOs
// =========================================================================== //

#[derive(Deserialize)]
pub struct AnthropicResponseDTO {
    pub id: String,
    pub content: Vec<AnthropicResponseContentBlockDTO>,
    pub stop_reason: Option<String>,
    pub usage: AnthropicUsageDTO,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicResponseContentBlockDTO {
    Text { 
        text: String 
    },
    ToolUse { 
        id: String, 
        name: String, 
        input: String,
    },
}

#[derive(Deserialize)]
pub struct AnthropicUsageDTO {
    pub input_tokens: usize,
    pub output_tokens: usize,
}
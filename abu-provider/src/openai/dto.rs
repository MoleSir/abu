use serde::{Deserialize, Serialize};
use abu_base::{chat::{AssistantMessage, ChatMessage, ChatRequest, ChatResponse, FinishReason, ToolCall, ToolDefinition}, common::{Role, Usage}};
use crate::ProvideResult;

// =========================================================================== //
//                              for send request 
// =========================================================================== //
#[derive(Serialize)]
pub struct OpenAiChatRequestDTO<'a> {
    messages: Vec<OpenAiMessageDTO<'a>>, 
    model: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiToolDefinitionDTO<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
}

#[derive(Serialize)]
pub struct OpenAiToolDefinitionDTO<'a> {
    r#type: &'static str, // "function"
    function: OpenAiFunctionDefinitionDTO<'a>,
}

#[derive(Serialize)]
pub struct OpenAiFunctionDefinitionDTO<'a> {
    name: &'a str,
    description: &'a str,
    parameters: &'a serde_json::Value,
}

#[derive(Serialize)]
pub struct OpenAiMessageDTO<'a> {
    role: Role,
    content: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<&'a str>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    tool_calls: Vec<OpenAiToolCallRefDTO<'a>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<&'a str>,
}

#[derive(Serialize)]
pub struct OpenAiToolCallRefDTO<'a> {
    pub id: &'a str,
    pub r#type: &'static str, // function
    pub function: OpenAiFunctionCallRefDTO<'a>,
}

#[derive(Serialize)]
pub struct OpenAiFunctionCallRefDTO<'a> {
    pub name: &'a str,
    pub arguments: &'a str,
}

impl<'a> OpenAiChatRequestDTO<'a> {
    pub fn from_request(req: &'a ChatRequest) -> Self {
        let tools_dto = req.tools.iter().map(|t| OpenAiToolDefinitionDTO {
            r#type: "function",
            function: OpenAiFunctionDefinitionDTO {
                name: &t.name,
                description: &t.description,
                parameters: &t.schema
            },
        }).collect();

        let messages_dto = req.messages.iter().map(|m| {
            let tool_calls_dto= if let ChatMessage::Assistant(m) = m {
                m.tool_calls.iter().map(|tc| OpenAiToolCallRefDTO {
                    id: &tc.id,
                    r#type: "function",
                    function: OpenAiFunctionCallRefDTO {
                        name: &tc.name,
                        arguments: &tc.arguments
                    },
                }).collect()
            } else {
                vec![]
            };

            let reasoning = if let ChatMessage::Assistant(m) = m {
                m.reasoning_content.as_deref()
            } else {
                None
            };

            OpenAiMessageDTO {
                role: m.role(),
                tool_call_id: if let ChatMessage::Tool(m) = m {
                    Some(&m.tool_call_id)
                } else { None  },
                content: m.content(),
                tool_calls: tool_calls_dto,
                reasoning_content: reasoning,
            }
        }).collect();

        Self {
            messages: messages_dto,
            model: &req.model,
            tools: tools_dto,
            temperature: req.temperature,
        }
    }
}

// =========================================================================== //
//                              for parse response 
// =========================================================================== //

#[derive(Deserialize)]
pub struct OpenAiChatResponseDTO {
    pub id: String,
    pub choices: Vec<OpenAiChatChoiceDTO>,
    pub usage: Usage
}

#[derive(Deserialize)]
pub struct OpenAiChatChoiceDTO {
    pub finish_reason: FinishReason,
    pub index: usize,
    pub message: OpenAiAssistantMessageDTO,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiEmbedResponseDTO {
    pub data: Vec<OpenAiEmbeddingDataDTO>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiEmbeddingDataDTO {
    pub embedding: Vec<f32>,
}

#[derive(Deserialize)]
pub struct OpenAiAssistantMessageDTO {
    #[serde(default)]
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<OpenAiToolCallDTO>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reasoning_content: Option<String>,
}

#[derive(Deserialize)]
pub struct OpenAiToolCallDTO {
    pub id: String,
    pub r#type: String,
    pub function: OpenAiFunctionCallDTO,
}

#[derive(Deserialize)]
pub struct OpenAiFunctionCallDTO {
    pub name: String,
    pub arguments: String,
}

impl OpenAiChatResponseDTO {
    pub fn to_chat_response(self) -> ProvideResult<ChatResponse> {
        let choice = self.choices.into_iter().next().ok_or_else(|| {
            crate::ProvideError::Api("OpenAI returned empty choices".into())
        })?;

        let tool_calls = choice.message.tool_calls.into_iter().map(|openao_tool_call| {
            ToolCall {
                id: openao_tool_call.id,
                name: openao_tool_call.function.name,
                arguments: openao_tool_call.function.arguments,
            }
        })
        .collect();

        let message = AssistantMessage {
            content: choice.message.content,
            tool_calls,
            name: None,
            reasoning_content: choice.message.reasoning_content,
        };

        Ok(ChatResponse {
            message,
            finish_reason: choice.finish_reason,
            usage: self.usage,
        })
    }
}

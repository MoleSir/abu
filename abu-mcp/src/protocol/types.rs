use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use derive_builder::Builder;

/// After receiving an initialize request from the client, the server sends this response.
#[derive(Debug, Clone, Serialize, Deserialize, Default, Builder)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInitializeResult {
    pub protocol_version: String,
    pub capabilities: McpServerCapabilities,
    pub server_info: McpImplementation,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Builder)]
#[serde(rename_all = "camelCase")]
pub struct McpClientInitializeResult {
    pub protocol_version: String,
    pub client_info: Option<McpImplementation>,
    pub capabilities: Option<McpClientCapabilities>,
}

/// Server capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default, Builder)]
#[serde(rename_all = "camelCase")]
pub struct McpServerCapabilities {
    /// Experimental, non-standard capabilities that the server supports.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub experimental: Option<HashMap<String, serde_json::Value>>,

    /// Present if the server supports sending log messages to the client.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub logging: Option<serde_json::Value>,

    /// Present if the server offers any prompt templates.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub prompts: Option<McpPromptsCapability>,

    /// Present if the server offers any resources to read.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub resources: Option<McpResourceCapability>,

    /// Present if the server offers any tools to call.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub tools: Option<McpToolsCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, Builder)]
#[serde(rename_all = "camelCase")]
pub struct McpPromptsCapability {
    /// Whether this server supports notifications for changes to the prompt list.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub list_changed: Option<bool>,
}

/// Resources capability
#[derive(Debug, Clone, Serialize, Deserialize, Default, Builder)]
#[serde(rename_all = "camelCase")]
pub struct McpResourceCapability {
    /// Whether this server supports subscribing to resource updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub subscribe: Option<bool>,

    /// Whether this server supports notifications for changes to the resource list.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub list_changed: Option<bool>,
}

/// Tools capability
#[derive(Debug, Clone, Serialize, Deserialize, Default, Builder)]
#[serde(rename_all = "camelCase")]
pub struct McpToolsCapability {
    /// Whether this server supports notifications for changes to the tool list.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub list_changed: Option<bool>,
}

#[derive(Debug, Serialize, Clone, Deserialize, Default, Builder)]
pub struct McpImplementation {
    pub name: String,
    pub version: String,
}

/// TODO: use struct!
#[derive(Debug, Serialize, Clone, Deserialize, Default)]
pub struct McpClientCapabilities {
    pub custom: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct McpTool {
    /// The name of the tool.
    pub name: String,

    /// A human-readable description of the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub description: Option<String>,

    /// A JSON Schema object defining the expected parameters for the tool.
    pub input_schema: McpToolInputSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct McpToolInputSchema {
    #[builder(default = "String::from(\"object\")")]
    pub r#type: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub properties: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default, setter(into))]
    pub required: Option<serde_json::Value>,
}


/// Reference https://modelcontextprotocol.io/specification/2024-11-05/server/tools
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct McpToolCall {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// TODO: Only support text yet!
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
pub struct McpToolCallResult {
    pub content: Vec<McpToolCallResultContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Tool result content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum McpToolCallResultContent {
    Text { text: String },
}

/// https://modelcontextprotocol.io/specification/2024-11-05/server/resources
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Multipurpose Internet Mail Extensions Type
    pub mime_type: String,
}

#[allow(unused)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_de_tool_call_result() {
        let res = McpToolCallResult{
            content: vec![ McpToolCallResultContent::Text { text: "molesir".to_string() } ],
            is_error: Some(false)
        };

        assert_eq!(
            serde_json::to_value(res).unwrap(),
            serde_json::json!(
                {
                    "content": [
                        {
                            "type": "text",
                            "text": "molesir"
                        }
                    ],
                    "is_error": false
                }
            )
        );
    }
}
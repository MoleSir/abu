use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// "type": "object",
    /// "properties": {
    ///     "content": {
    ///         "type": "string",
    ///         "description": "The text to echo"
    ///     }
    /// },
    /// "required": ["content"],
    pub schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
	pub id: String,
	pub name: String,
    pub arguments: String,
}


impl ToolDefinition {
    pub fn new(name: impl Into<String>, description: impl Into<String>, schema: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            schema
        }
    }
}
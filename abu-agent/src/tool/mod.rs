pub mod fs;
pub mod calculate;
pub mod terminate;
pub mod bash;
pub mod skill;

use abu_api::chat::{FunctionInfo, ToolCall, ToolDefinition, ToolType};
use abu_mcp::{McpTool, McpToolInputSchema};
use std::collections::HashMap;
use serde_json::Value;
use async_trait::async_trait;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Vec<ToolParameter>;
    async fn execute(&self, args: Value) -> ToolResult<ToolCallResult>;

    fn to_function_define(&self) -> ToolDefinition {
        let schema = parameters_schema(&self.parameters());
        ToolDefinition {
            r#type: ToolType::Function,
            function: FunctionInfo {
                description: self.description().to_string(),
                name: self.name().to_string(),
                parameters: schema,
            }
        }
    }

    fn to_mcptool(&self) -> McpTool {
        let schema = mcptool_input_schema(&self.parameters());
        McpTool {
            name: self.name().to_string(),
            description: Some(self.description().to_string()),
            input_schema: schema,
        }
    }
}

pub fn tool_str_arg<'a, 'b>(args: &'a Value, name: &'b str) -> ToolResult<&'a str> {
    args
        .get(name)
        .ok_or_else(|| ToolError::ArgNotFound(name.to_string()))?
        .as_str().ok_or_else(|| ToolError::ArgParse("string"))
}

fn mcptool_input_schema(params: &[ToolParameter]) -> McpToolInputSchema {
    let (properties, requireds) = parameters_info(params);
    McpToolInputSchema {
        r#type: "object".to_string(),
        properties: Some(serde_json::json!(properties)),
        required: Some(serde_json::json!(requireds)),
    }
}

fn parameters_schema(params: &[ToolParameter]) -> serde_json::Value {
    let (properties, requireds) = parameters_info(params);
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": requireds,
    })
}

fn parameters_info(params: &[ToolParameter]) -> (HashMap<String, Value>, Vec<String>) {
    let mut properties = HashMap::new();
    let mut requireds = vec![];

    for param in params {
        if param.required {
            requireds.push(param.name.clone());
        }
        properties.insert(param.name.clone(), param.to_schema());
    }

    (properties, requireds)
}

pub struct ToolParameter {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
    pub kind: ToolParameterKind,
}

pub enum ToolParameterKind {
    Object(Vec<ToolParameter>),
    Array(Box<ToolParameterKind>),
    String(Option<Vec<&'static str>>),
    Integer,
    Number,
    Boolean,
}

impl ToolParameter {
    pub fn integer(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: true,
            description: None,
            kind: ToolParameterKind::Integer
        }
    }

    pub fn number(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: true,
            description: None,
            kind: ToolParameterKind::Number
        }
    }

    pub fn string(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: true,
            description: None,
            kind: ToolParameterKind::String(None)
        }
    }

    pub fn string_with(name: impl Into<String>, enums: Vec<&'static str>) -> Self {
        Self {
            name: name.into(),
            required: true,
            description: None,
            kind: ToolParameterKind::String(Some(enums)),
        }
    }

    pub fn array(name: impl Into<String>, kind: ToolParameterKind) -> Self {
        Self {
            name: name.into(),
            required: true,
            description: None,
            kind: ToolParameterKind::Array(Box::new(kind))
        }
    }

    pub fn object(name: impl Into<String>, field: Vec<ToolParameter>) -> Self {
        Self {
            name: name.into(),
            required: true,
            description: None,
            kind: ToolParameterKind::Object(field),
        }
    }

    pub fn required(self, value: bool) -> Self {
        Self { required: value, ..self }
    }

    pub fn description(self, value: impl Into<String>) -> Self {
        Self { description: Some(value.into()), ..self }
    }

    pub fn to_schema(&self) -> serde_json::Value {
        let mut schema = self.kind.to_schema();
        if let Some(desc) = self.description.as_ref() {
            schema["description"] = serde_json::Value::String(desc.to_string());
        }
        schema
    }
}

impl ToolParameterKind {
    pub fn to_schema(&self) -> serde_json::Value {
        match &self {
            Self::Object(params) => parameters_schema(&params),
            Self::Array(kind) => serde_json::json!({
                "type": "array",
                "items": kind.to_schema(),
            }),
            Self::String(enums) => match enums {
                Some(enums) => serde_json::json!({ "type": "string", "enums": enums }),
                None => serde_json::json!({ "type": "string" }),
            }
            Self::Boolean => serde_json::json!({ "type": "boolean" }),
            Self::Number => serde_json::json!({ "type": "number" }),
            Self::Integer => serde_json::json!({ "type": "integer" }),
        }
    }
}

pub struct ToolCollection {
    tools: HashMap<&'static str , Box<dyn Tool>>,
}

impl ToolCollection {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn init<I: Iterator<Item = Box<dyn Tool>>>(tools: I) -> Self {
        Self {
            tools: tools.map(|tool| (tool.name(), tool)).collect()
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn get_tool(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn add_tool<T: Tool + 'static>(&mut self, tool: T) {
        let tool = Box::new(tool);
        self.add_tool_box(tool);
    }

    pub fn add_tool_box(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name();
        self.tools.insert(name, tool);
    }

    pub async fn execute_tool(&self, name: &str, arguments: serde_json::Value) -> ToolResult<ToolCallResult> {
        let tool = self.get_tool(name).ok_or_else(|| ToolError::ToolNotFound(name.to_string()))?;
        let value = tool.execute(arguments).await?;
        Ok(value)
    }

    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.tools.contains_key(tool_name)
    }

    /// Return tool execute error if tool inner error
    pub async fn execute_toolcall(&self, tool_call: &ToolCall) -> ToolResult<ToolCallResult> {
        let functioncall = &tool_call.function;
        let arguments: serde_json::Value = serde_json::from_str(&functioncall.arguments)?;
        self.execute_tool(&functioncall.name, arguments).await
    }

    pub fn to_function_defines(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|(_, tool)| tool.to_function_define()).collect()
    }

    pub fn add_tools<I: Iterator<Item = Box<dyn Tool>>>(&mut self, tools: I) {
        for tool in tools {
            self.add_tool_box(tool);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolCallResult {
    pub is_error: bool,
    pub context: String,
}

impl ToolCallResult {
    pub fn error(context: impl Into<String>) -> Self {
        Self { is_error: true, context: context.into() }
    }

    pub fn success(context: impl Into<String>) -> Self {
        Self { is_error: false, context: context.into() }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error("tool {0} not found")]
    ToolNotFound(String),

    #[error("arg {0} not found")]
    ArgNotFound(String),

    #[error("arg parse failed, expect: Expect {0}")]
    ArgParse(&'static str),
}

pub type ToolResult<T> = std::result::Result<T, ToolError>; 

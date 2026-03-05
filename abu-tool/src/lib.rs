mod register;
pub use register::ToolRegister;

use abu_api::chat::{FunctionInfo, ToolDefinition, ToolType};
use serde_json::Value;
use std::collections::HashMap;

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Vec<ToolParameter>;
    async fn execute(&self, args: Value) -> ToolResult<ToolCallResult>;

    fn to_function_define(&self) -> ToolDefinition {
        let schema = ToolParameter::parameters_schema(&self.parameters());
        ToolDefinition {
            r#type: ToolType::Function,
            function: FunctionInfo {
                description: self.description().to_string(),
                name: self.name().to_string(),
                parameters: schema,
            }
        }
    }
}

pub struct ToolParameter {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
    pub kind: ToolParameterKind,
}

pub struct ToolParametersInfo {
    pub properties: HashMap<String, serde_json::Value>,
    pub required: Vec<String>,
}

pub enum ToolParameterKind {
    Object(Vec<ToolParameter>),
    Array(Box<ToolParameterKind>),
    String(Option<Vec<&'static str>>),
    Integer,
    Number,
    Boolean,
}

#[derive(Debug, Clone)]
pub struct ToolCallResult {
    pub is_error: bool,
    pub context: String,
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

    pub fn extract_info(params: &[ToolParameter]) -> ToolParametersInfo {
        let mut properties = HashMap::new();
        let mut required = vec![];
    
        for param in params {
            if param.required {
                required.push(param.name.clone());
            }
            properties.insert(param.name.clone(), param.to_schema());
        }
    
        ToolParametersInfo { properties, required }
    }
    
    pub fn parameters_schema(params: &[ToolParameter]) -> serde_json::Value {
        let info = Self::extract_info(params);
        serde_json::json!({
            "type": "object",
            "properties": info.properties,
            "required": info.required,
        })
    }
}

impl ToolParameterKind {
    pub fn to_schema(&self) -> serde_json::Value {
        match &self {
            Self::Object(params) => ToolParameter::parameters_schema(&params),
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

impl ToolCallResult {
    pub fn error(context: impl Into<String>) -> Self {
        Self { is_error: true, context: context.into() }
    }

    pub fn success(context: impl Into<String>) -> Self {
        Self { is_error: false, context: context.into() }
    }

    pub fn display(&self) -> String {
        if self.is_error {
            format!("tool execeute failed: {}", self.context)
        } else {
            format!("tool execeute success: {}", self.context)
        }
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

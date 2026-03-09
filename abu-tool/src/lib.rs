mod register;
pub use register::ToolRegister;
pub use abu_base::chat::ToolDefinition;
use serde_json::Value;

#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Vec<ToolParameter>;
    async fn execute(&self, args: Value) -> ToolResult<ToolCallResult>;

    fn to_function_define(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            schema: build_params_schema(&self.parameters())
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolParameter {
    pub name: String,
    pub required: bool,
    pub description: Option<String>,
    pub kind: ToolParameterKind,
}

#[derive(Debug, Clone)]
pub enum ToolParameterKind {
    Object(Vec<ToolParameter>),
    Array(Box<ToolParameterKind>),
    String(Option<Vec<String>>),
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

    pub fn string_with(name: impl Into<String>, enums: Vec<String>) -> Self {
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

    pub fn build_params_properties(params: &[ToolParameter]) -> serde_json::Map<String, serde_json::Value> {
        let mut properties = serde_json::Map::new();
        for param in params {
            let mut param_schema = param.kind.to_schema();        
            if let Some(desc) = &param.description {
                param_schema["description"] = serde_json::json!(desc);
            }
            
            properties.insert(param.name.clone(), param_schema);
        }
        properties
    }
    
    pub fn build_params_required(params: &[ToolParameter]) -> Vec<String> {
        params.iter()
            .filter(|p| p.required)
            .map(|p| p.name.clone())
            .collect()
    }
}

impl ToolParameterKind {
    pub fn to_schema(&self) -> serde_json::Value {
        match &self {
            Self::Object(params) => build_params_schema(&params),
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

pub(crate) fn build_params_schema(params: &[ToolParameter]) -> serde_json::Value {
    let properties = ToolParameter::build_params_properties(params);
    let required = ToolParameter::build_params_required(params);
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
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

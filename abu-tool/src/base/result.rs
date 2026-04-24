use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
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



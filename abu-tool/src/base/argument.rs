use serde::de::DeserializeOwned;
use super::{ToolError, ToolParameterKind};

pub trait ToolArgument: DeserializeOwned {
    fn parameter_kind() -> ToolParameterKind;
    fn from_value(value: serde_json::Value) -> Result<Self, ToolError> {
        serde_json::from_value(value).map_err(ToolError::SerdeJson)
    }
}

impl ToolArgument for String {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::String(None) }
}

impl ToolArgument for i64 {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::Integer }
}

impl ToolArgument for u64 {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::Integer }
}

impl ToolArgument for usize {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::Integer }
}

impl ToolArgument for f64 {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::Number }
}

impl ToolArgument for i32 {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::Integer }
}

impl ToolArgument for u32 {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::Integer }
}

impl ToolArgument for i8 {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::Integer }
}

impl ToolArgument for u8 {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::Integer }
}

impl ToolArgument for bool {
    fn parameter_kind() -> ToolParameterKind { ToolParameterKind::Boolean }
}

impl<T: ToolArgument> ToolArgument for Vec<T> {
    fn parameter_kind() -> ToolParameterKind {
        ToolParameterKind::Array(Box::new(T::parameter_kind()))
    }
}

impl<T: ToolArgument> ToolArgument for Option<T> {
    fn parameter_kind() -> ToolParameterKind {
        T::parameter_kind()
    }
}

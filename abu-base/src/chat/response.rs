use crate::common::Usage;

use super::message::*;
use serde::Deserialize;
use strum::{Display, EnumString, EnumVariantNames};

#[derive(Debug, Clone, Deserialize)]
pub struct ChatResponse {
    pub message: AssistantMessage,
    pub finish_reason: FinishReason,
    pub usage: Usage,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, EnumString, Display, EnumVariantNames)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    #[default]
    Stop,
    Length,
    ContentFilter,
    ToolCalls,
}

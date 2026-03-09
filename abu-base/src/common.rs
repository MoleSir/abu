use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Usage {
    /// Number of tokens in the generated completion.
    pub completion_tokens: usize,
    /// Number of tokens in the prompt.
    pub prompt_tokens: usize,
    /// Total number of tokens used in the request (prompt + completion).
    pub total_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}
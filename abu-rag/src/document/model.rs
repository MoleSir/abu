use std::{fmt::Display, path::PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(pub String); 

#[derive(Debug, Clone)]
pub struct Document {
    pub id: Id,
    pub source: PathBuf,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: Id,
    pub document_id: Id,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

impl Id {
    pub fn new(inner: impl Into<String>) -> Self {
        Self(inner.into())
    }

    pub fn uuid() -> Self {
        Self::new(uuid::Uuid::new_v4())
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
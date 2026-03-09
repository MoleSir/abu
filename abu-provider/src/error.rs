#[derive(Debug, thiserror::Error)]
pub enum ProvideError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("API error: {0}")]
    Api(String), 
    
    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    EnvVar(#[from] std::env::VarError),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    
    #[error("Except messgae {0}")]
    ExceptMessage(&'static str),
}

pub type ProvideResult<T> = std::result::Result<T, ProvideError>;
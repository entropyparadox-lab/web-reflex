use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Failed to parse HTML: {0}")]
    HtmlParseError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Regex compilation error: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Invalid Action Node: {0}")]
    ValidationError(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

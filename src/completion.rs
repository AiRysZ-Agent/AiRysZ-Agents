// src/completion.rs
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum CompletionError {
    ApiError(String),
    Other(Box<dyn Error + Send + Sync>), // Ensure the inner error is Send + Sync
}

impl fmt::Display for CompletionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompletionError::ApiError(msg) => write!(f, "API Error: {}", msg),
            CompletionError::Other(err) => write!(f, "Error: {}", err),
        }
    }
}

impl Error for CompletionError {}

impl From<reqwest::Error> for CompletionError {
    fn from(err: reqwest::Error) -> Self {
        CompletionError::Other(Box::new(err))
    }
}

#[async_trait::async_trait]
pub trait CompletionProvider {
    type Error: Error + Send + Sync + 'static;

    async fn complete(&self, prompt: &str) -> Result<String, Self::Error>;
}

use thiserror::Error;

/// Result type for Cerberus operations.
pub type CerberusResult<T> = Result<T, CerberusError>;

/// Core error types for the Cerberus application.
#[derive(Debug, Error)]
pub enum CerberusError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Settings persistence error.
    #[error("Persistence error: {0}")]
    Persistence(String),

    /// Settings validation error.
    #[error("Validation error: {0}")]
    Validation(String),

    /// Migration error.
    #[error("Migration error: {0}")]
    Migration(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Generic error.
    #[error("{0}")]
    Other(String),
}

impl CerberusError {
    /// Create a persistence error.
    pub fn persistence<S: Into<String>>(msg: S) -> Self {
        Self::Persistence(msg.into())
    }

    /// Create a validation error.
    pub fn validation<S: Into<String>>(msg: S) -> Self {
        Self::Validation(msg.into())
    }

    /// Create a migration error.
    pub fn migration<S: Into<String>>(msg: S) -> Self {
        Self::Migration(msg.into())
    }

    /// Create a config error.
    pub fn config<S: Into<String>>(msg: S) -> Self {
        Self::Config(msg.into())
    }
}

impl From<anyhow::Error> for CerberusError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e.to_string())
    }
}

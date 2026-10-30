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

    /// rclone not found
    #[error("rclone not found. Please install rclone from https://rclone.org/downloads/")]
    RcloneNotFound,

    /// rclone error
    #[error("rclone error: {0}")]
    RcloneError(String),

    /// WinFsp not found
    #[error("WinFsp not found. Install from https://winfsp.dev/")]
    WinFspNotFound,

    /// Mount error
    #[error("Mount error: {0}")]
    MountError(String),

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

    /// Create rclone not found error
    pub fn rclone_not_found() -> Self {
        Self::RcloneNotFound
    }

    /// Create rclone error
    pub fn rclone_error<S: Into<String>>(msg: S) -> Self {
        Self::RcloneError(msg.into())
    }

    /// Create WinFsp not found error
    pub fn winfsp_not_found() -> Self {
        Self::WinFspNotFound
    }

    /// Create mount error
    pub fn mount_error<S: Into<String>>(msg: S) -> Self {
        Self::MountError(msg.into())
    }
}

impl From<anyhow::Error> for CerberusError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e.to_string())
    }
}

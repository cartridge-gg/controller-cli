use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Session not found. Run 'controller generate' and 'controller register' first")]
    NoSession,

    #[error("Session expired at {0}. Run 'controller register' to create a new session")]
    SessionExpired(String),

    #[error("Policy violation: {message}")]
    #[allow(dead_code)] // Reserved for future policy validation
    PolicyViolation { message: String, details: String },

    #[error("Invalid session data: {0}")]
    InvalidSessionData(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Network error: {0}")]
    #[allow(dead_code)] // Reserved for network-related errors
    Network(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Callback timeout: No authorization received within {0} seconds")]
    CallbackTimeout(u64),

    #[error("Server error: {0}")]
    #[allow(dead_code)] // Reserved for server-related errors
    ServerError(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Timeout: {0}")]
    TimeoutError(String),

    #[error("Not found: {0}")]
    NotFoundError(String),

    #[error("File error for {path}: {message}")]
    FileError { path: String, message: String },

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl CliError {
    pub fn error_code(&self) -> &'static str {
        match self {
            CliError::NoSession => "NoSession",
            CliError::SessionExpired(_) => "SessionExpired",
            CliError::PolicyViolation { .. } => "PolicyViolation",
            CliError::InvalidSessionData(_) => "InvalidSessionData",
            CliError::Storage(_) => "StorageError",
            CliError::Network(_) => "NetworkError",
            CliError::TransactionFailed(_) => "TransactionFailed",
            CliError::InvalidInput(_) => "InvalidInput",
            CliError::CallbackTimeout(_) => "CallbackTimeout",
            CliError::ServerError(_) => "ServerError",
            CliError::ApiError(_) => "ApiError",
            CliError::TimeoutError(_) => "TimeoutError",
            CliError::NotFoundError(_) => "NotFoundError",
            CliError::FileError { .. } => "FileError",
            CliError::Other(_) => "UnknownError",
        }
    }

    pub fn recovery_hint(&self) -> Option<&'static str> {
        match self {
            CliError::NoSession => Some(
                "Run 'controller generate' followed by 'controller register' to set up a session",
            ),
            CliError::SessionExpired(_) => {
                Some("Run 'controller register' to create a new session")
            }
            CliError::PolicyViolation { .. } => {
                Some("Review your session policies or register a new session with updated policies")
            }
            CliError::CallbackTimeout(_) => Some("Try running register again"),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, CliError>;

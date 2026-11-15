//! Framework-free failure vocabulary shared by domain and application layers.
use thiserror::Error;
/// Safe errors contain no SQL, credentials, or arbitrary upstream payloads.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AppError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("authentication required")]
    Unauthorized,
    #[error("access denied")]
    Forbidden,
    #[error("resource not found")]
    NotFound,
    #[error("{0}")]
    Conflict(&'static str),
    #[error("dependency unavailable")]
    Unavailable,
    #[error("internal failure")]
    Internal,
}
pub type AppResult<T> = Result<T, AppError>;

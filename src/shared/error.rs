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
    #[error("{code}")]
    ConflictDetails {
        code: &'static str,
        details: ConflictDetails,
    },
    #[error("dependency unavailable")]
    Unavailable,
    #[error("internal failure")]
    Internal,
}
pub type AppResult<T> = Result<T, AppError>;

/// Neutral conflict facts; transport chooses field names and envelope formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictDetails {
    AnswerRevision {
        snapshot_question_id: uuid::Uuid,
        option_ids: Vec<uuid::Uuid>,
        revision: i64,
        saved_at: Option<chrono::DateTime<chrono::Utc>>,
    },
}

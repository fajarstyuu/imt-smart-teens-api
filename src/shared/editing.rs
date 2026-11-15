//! Framework-free editorial metadata, pagination and plain-text validation shared by contexts.
use crate::shared::error::{AppError, AppResult};
use chrono::{DateTime, Utc};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentState {
    Draft,
    Published,
    Archived,
}
impl ContentState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Archived => "archived",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "draft" => Ok(Self::Draft),
            "published" => Ok(Self::Published),
            "archived" => Ok(Self::Archived),
            _ => Err(AppError::Invalid("INVALID_CONTENT_STATUS")),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Version {
    pub id: Uuid,
    pub version: i32,
    pub published_at: DateTime<Utc>,
}
/// Content is plain text. Markup delimiters are rejected rather than trusted as admin HTML.
pub fn text(s: &str, max: usize, empty: bool) -> AppResult<()> {
    if (!empty && s.trim().is_empty())
        || s.chars().count() > max
        || s.contains(['<', '>'])
        || s.chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err(AppError::Invalid("INVALID_CONTENT_TEXT"));
    }
    Ok(())
}
pub struct ContentQuery {
    pub status: Option<ContentState>,
    pub limit: i64,
    pub offset: i64,
}
impl ContentQuery {
    pub fn validate(&self) -> AppResult<()> {
        if !(1..=100).contains(&self.limit) || self.offset < 0 {
            Err(AppError::Invalid("INVALID_PAGINATION"))
        } else {
            Ok(())
        }
    }
}
pub struct Page<T> {
    pub rows: Vec<T>,
    pub total: i64,
}
pub fn revision(r: i64) -> AppResult<()> {
    if r < 0 {
        Err(AppError::Invalid("INVALID_REVISION"))
    } else {
        Ok(())
    }
}
pub fn check_revision(actual: i64, expected: i64) -> AppResult<()> {
    if actual != expected {
        Err(AppError::Conflict("CONTENT_REVISION_CONFLICT"))
    } else {
        Ok(())
    }
}

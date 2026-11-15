//! Shared admin editorial query/revision/version DTOs, independent of context persistence.
use crate::{
    shared::editing::{ContentQuery, ContentState, Version},
    transport::http::error::ApiError,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Explicit success envelope; no untyped JSON schema at the handler boundary.
#[derive(Serialize)]
pub struct Data<T> {
    pub data: T,
}
#[derive(Serialize)]
pub struct PageMeta {
    pub limit: i64,
    pub offset: i64,
    pub total: i64,
}
#[derive(Serialize)]
pub struct PageDto<T> {
    pub data: Vec<T>,
    pub meta: PageMeta,
}
#[derive(Serialize)]
pub struct VersionDto {
    pub id: Uuid,
    pub version: i32,
    pub published_at: DateTime<Utc>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
impl ListQuery {
    pub fn core(self) -> Result<ContentQuery, ApiError> {
        Ok(ContentQuery {
            status: self.status.map(|s| ContentState::parse(&s)).transpose()?,
            limit: self.limit.unwrap_or(25),
            offset: self.offset.unwrap_or(0),
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub expected_revision: i64,
}
pub fn version_dto(v: Version) -> VersionDto {
    VersionDto {
        id: v.id,
        version: v.version,
        published_at: v.published_at,
    }
}

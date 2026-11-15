//! Shared admin editorial query/revision/version DTOs, independent of context persistence.
use crate::{
    shared::editing::{ContentQuery, ContentState, Version},
    transport::http::error::ApiError,
};
use serde::Deserialize;
use serde_json::{Value, json};
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
pub fn version_dto(v: Version) -> Value {
    json!({"id":v.id,"version":v.version,"published_at":v.published_at})
}

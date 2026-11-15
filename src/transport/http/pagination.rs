//! Bounded offset pagination, reusable by catalog and reporting endpoints.
use crate::shared::application::{AppError, AppResult};
use serde::Deserialize;
#[derive(Default, Deserialize)]
pub struct Pagination {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
impl Pagination {
    pub fn validate(&self) -> AppResult<(i64, i64)> {
        let limit = self.limit.unwrap_or(25);
        let offset = self.offset.unwrap_or(0);
        if !(1..=100).contains(&limit) || offset < 0 {
            return Err(AppError::Invalid("INVALID_PAGINATION"));
        }
        Ok((limit, offset))
    }
}

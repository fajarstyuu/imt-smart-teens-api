//! Public allowlisted response types do not serialize database rows or password fields.
use crate::modules::identity::{
    application::{School, SchoolQuery, SchoolStatus},
    domain::{AdminPrincipal, SchoolName},
};
use crate::{
    shared::application::{AppError, AppResult},
    transport::http::pagination::Pagination,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Serialize)]
pub struct AdminDto {
    id: Uuid,
    name: String,
    email: String,
    can_manage_schools: bool,
    can_manage_content: bool,
    school_ids: Vec<Uuid>,
}
impl From<AdminPrincipal> for AdminDto {
    fn from(a: AdminPrincipal) -> Self {
        Self {
            id: a.id,
            name: a.name,
            email: a.email,
            // Deprecated compatibility fields describe global capabilities,
            // never a list of schools the caller must be granted separately.
            can_manage_schools: true,
            can_manage_content: true,
            school_ids: Vec::new(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}
#[derive(Serialize)]
pub struct SchoolDto {
    id: Uuid,
    name: String,
    status: &'static str,
    revision: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    archived_at: Option<DateTime<Utc>>,
}
impl From<School> for SchoolDto {
    fn from(s: School) -> Self {
        Self {
            id: s.id,
            name: s.name,
            status: if s.archived_at.is_some() {
                "archived"
            } else {
                "active"
            },
            revision: s.revision,
            created_at: s.created_at,
            updated_at: s.updated_at,
            archived_at: s.archived_at,
        }
    }
}
#[derive(Serialize)]
pub struct PublicSchoolDto {
    id: Uuid,
    name: String,
}
impl From<School> for PublicSchoolDto {
    fn from(s: School) -> Self {
        Self {
            id: s.id,
            name: s.name,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSchool {
    pub name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameSchool {
    pub name: String,
    pub expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveSchool {
    pub expected_revision: i64,
}
#[derive(Deserialize, Default)]
pub struct SchoolListQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
impl SchoolListQuery {
    pub fn into_query(self, public: bool) -> AppResult<SchoolQuery> {
        let (limit, offset) = Pagination {
            limit: self.limit,
            offset: self.offset,
        }
        .validate()?;
        let status = if public {
            SchoolStatus::Active
        } else {
            match self.status.as_deref().unwrap_or("active") {
                "active" => SchoolStatus::Active,
                "archived" => SchoolStatus::Archived,
                "all" => SchoolStatus::All,
                _ => return Err(AppError::Invalid("INVALID_SCHOOL_STATUS")),
            }
        };
        let q = self.q.unwrap_or_default();
        if q.chars().count() > 300 {
            return Err(AppError::Invalid("INVALID_SEARCH"));
        }
        let normalized_search = if q.trim().is_empty() {
            String::new()
        } else {
            SchoolName::parse(&q)?.normalized().to_owned()
        };
        Ok(SchoolQuery {
            normalized_search,
            status,
            limit,
            offset,
        })
    }
}

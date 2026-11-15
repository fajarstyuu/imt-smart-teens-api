//! Catalog use cases enforce permissions separately from reporting school grants.
use crate::{
    modules::identity::domain::{Principal, SchoolName},
    shared::application::{AppError, AppResult},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct School {
    pub id: Uuid,
    pub name: String,
    pub revision: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
}
pub struct SchoolQuery {
    pub normalized_search: String,
    pub status: SchoolStatus,
    pub limit: i64,
    pub offset: i64,
}
#[derive(Clone, Copy)]
pub enum SchoolStatus {
    Active,
    Archived,
    All,
}
pub struct SchoolPage {
    pub schools: Vec<School>,
    pub total: i64,
}
/// Audit actor is verified at the application boundary, never mass-assigned from JSON.
pub struct AuditActor {
    pub admin_id: Uuid,
    pub session_id: Uuid,
    pub request_id: String,
}
#[async_trait]
pub trait SchoolStore: Send + Sync {
    async fn list(&self, query: &SchoolQuery) -> AppResult<SchoolPage>;
    async fn get(&self, id: Uuid) -> AppResult<School>;
    async fn create(&self, name: SchoolName, actor: AuditActor) -> AppResult<School>;
    async fn rename(
        &self,
        id: Uuid,
        name: SchoolName,
        expected_revision: i64,
        actor: AuditActor,
    ) -> AppResult<School>;
    async fn archive(
        &self,
        id: Uuid,
        expected_revision: i64,
        actor: AuditActor,
    ) -> AppResult<School>;
}
#[derive(Clone)]
pub struct SchoolService {
    store: Arc<dyn SchoolStore>,
}
impl SchoolService {
    pub fn new(store: Arc<dyn SchoolStore>) -> Self {
        Self { store }
    }
    pub async fn public_list(&self, query: SchoolQuery) -> AppResult<SchoolPage> {
        if !matches!(query.status, SchoolStatus::Active) {
            return Err(AppError::Forbidden);
        }
        self.store.list(&query).await
    }
    pub async fn list(&self, principal: &Principal, query: SchoolQuery) -> AppResult<SchoolPage> {
        principal.admin()?.require_schools()?;
        self.store.list(&query).await
    }
    pub async fn get(&self, principal: &Principal, id: Uuid) -> AppResult<School> {
        principal.admin()?.require_schools()?;
        self.store.get(id).await
    }
    pub async fn create(
        &self,
        principal: &Principal,
        session: Uuid,
        request_id: String,
        name: &str,
    ) -> AppResult<School> {
        let actor = actor(principal, session, request_id)?;
        self.store.create(SchoolName::parse(name)?, actor).await
    }
    pub async fn rename(
        &self,
        principal: &Principal,
        session: Uuid,
        request_id: String,
        id: Uuid,
        name: &str,
        revision: i64,
    ) -> AppResult<School> {
        let actor = actor(principal, session, request_id)?;
        if revision < 0 {
            return Err(AppError::Invalid("INVALID_REVISION"));
        }
        self.store
            .rename(id, SchoolName::parse(name)?, revision, actor)
            .await
    }
    pub async fn archive(
        &self,
        principal: &Principal,
        session: Uuid,
        request_id: String,
        id: Uuid,
        revision: i64,
    ) -> AppResult<School> {
        let actor = actor(principal, session, request_id)?;
        if revision < 0 {
            return Err(AppError::Invalid("INVALID_REVISION"));
        }
        self.store.archive(id, revision, actor).await
    }
}
fn actor(principal: &Principal, session_id: Uuid, request_id: String) -> AppResult<AuditActor> {
    let admin = principal.admin()?;
    admin.require_schools()?;
    Ok(AuditActor {
        admin_id: admin.id,
        session_id,
        request_id,
    })
}

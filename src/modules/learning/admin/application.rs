//! Typed editorial commands and policies; draft identities never become published row IDs.
mod publication;
pub use crate::shared::editing::{
    ContentQuery, ContentState, Page, Version, check_revision, revision, text,
};
use crate::{
    modules::identity::{application::AuditActor, domain::Principal},
    shared::error::{AppError, AppResult},
};
use async_trait::async_trait;
pub use publication::{
    PathPublication, PathPublicationTransaction, PublishedLesson, PublishedLessonVersion,
    publish_path_in_transaction,
};
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;
#[derive(Clone, Debug)]
pub struct SectionDraft {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub required: bool,
}
#[derive(Clone, Debug)]
pub struct LessonDraft {
    pub title: String,
    pub description: String,
    pub sections: Vec<SectionDraft>,
}
impl LessonDraft {
    pub fn validate(&self, publishing: bool) -> AppResult<()> {
        text(&self.title, 200, false)?;
        text(&self.description, 10000, true)?;
        let mut ids = HashSet::new();
        for s in &self.sections {
            if !ids.insert(s.id) {
                return Err(AppError::Invalid("DUPLICATE_SECTION_ID"));
            }
            text(&s.title, 200, false)?;
            text(&s.body, 10000, !publishing)?;
        }
        if self.sections.len() > 100 || (publishing && self.sections.is_empty()) {
            return Err(AppError::Invalid("INVALID_LESSON_SECTIONS"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct LessonEditor {
    pub id: Uuid,
    pub state: ContentState,
    pub revision: i64,
    pub position: Option<i32>,
    pub draft: LessonDraft,
    pub versions: Vec<Version>,
}
#[derive(Clone, Debug)]
pub struct PathEditor {
    pub id: Uuid,
    pub revision: i64,
    pub lesson_ids: Vec<Uuid>,
    pub paired_bank_id: Uuid,
    pub quiz_bank_id: Uuid,
    pub versions: Vec<Version>,
    pub published: Option<PathManifest>,
}
#[derive(Clone, Debug)]
pub struct PathManifest {
    pub id: Uuid,
    pub version: i32,
    pub paired_bank_version_id: Uuid,
    pub quiz_bank_version_id: Uuid,
    pub lessons: Vec<(Uuid, Uuid, i32)>,
}
/// The adapter owns one transaction; path is always locked before lesson roots.
#[async_trait]
pub trait LessonStore: Send + Sync {
    /// Explicit private bootstrap; preserve all existing content and only add missing empty roots.
    async fn initialize_empty(&self) -> AppResult<()>;
    async fn list(&self, q: ContentQuery) -> AppResult<Page<LessonEditor>>;
    async fn get(&self, id: Uuid) -> AppResult<LessonEditor>;
    async fn create(
        &self,
        draft: LessonDraft,
        position: i32,
        actor: AuditActor,
    ) -> AppResult<LessonEditor>;
    async fn update(
        &self,
        id: Uuid,
        draft: LessonDraft,
        position: i32,
        revision: i64,
        actor: AuditActor,
    ) -> AppResult<LessonEditor>;
    async fn publish(&self, id: Uuid, revision: i64, actor: AuditActor) -> AppResult<LessonEditor>;
    async fn archive(&self, id: Uuid, revision: i64, actor: AuditActor) -> AppResult<LessonEditor>;
    async fn begin_path_publication(&self) -> AppResult<Box<dyn PathPublicationTransaction>>;
    async fn path(&self) -> AppResult<PathEditor>;
    async fn order(
        &self,
        ids: Vec<Uuid>,
        revision: i64,
        actor: AuditActor,
    ) -> AppResult<PathEditor>;
}
#[derive(Clone)]
pub struct LessonCms {
    store: Arc<dyn LessonStore>,
}
impl LessonCms {
    pub fn new(store: Arc<dyn LessonStore>) -> Self {
        Self { store }
    }
    /// No startup or public HTTP caller invokes this private operational use case.
    pub async fn initialize_development(&self, environment: &str) -> AppResult<()> {
        if environment != "development" {
            return Err(AppError::Forbidden);
        }
        self.store.initialize_empty().await
    }
    pub async fn list(&self, p: &Principal, q: ContentQuery) -> AppResult<Page<LessonEditor>> {
        authorize(p)?;
        q.validate()?;
        self.store.list(q).await
    }
    pub async fn get(&self, p: &Principal, id: Uuid) -> AppResult<LessonEditor> {
        authorize(p)?;
        self.store.get(id).await
    }
    pub async fn create(
        &self,
        p: &Principal,
        session: Uuid,
        request: String,
        d: LessonDraft,
        pos: i32,
    ) -> AppResult<LessonEditor> {
        let actor = actor(p, session, request)?;
        d.validate(false)?;
        if pos < 1 {
            return Err(AppError::Invalid("INVALID_POSITION"));
        }
        self.store.create(d, pos, actor).await
    }
    pub async fn update(
        &self,
        p: &Principal,
        a: AuditActor,
        id: Uuid,
        d: LessonDraft,
        pos: i32,
        rev: i64,
    ) -> AppResult<LessonEditor> {
        authorize(p)?;
        revision(rev)?;
        d.validate(false)?;
        self.store.update(id, d, pos, rev, a).await
    }
    pub async fn publish(
        &self,
        p: &Principal,
        a: AuditActor,
        id: Uuid,
        rev: i64,
    ) -> AppResult<LessonEditor> {
        authorize(p)?;
        revision(rev)?;
        self.store.publish(id, rev, a).await
    }
    pub async fn archive(
        &self,
        p: &Principal,
        a: AuditActor,
        id: Uuid,
        rev: i64,
    ) -> AppResult<LessonEditor> {
        authorize(p)?;
        revision(rev)?;
        self.store.archive(id, rev, a).await
    }
    pub async fn publish_path(
        &self,
        p: &Principal,
        a: AuditActor,
        rev: i64,
    ) -> AppResult<PathEditor> {
        authorize(p)?;
        revision(rev)?;
        publish_path_in_transaction(self.store.begin_path_publication().await?, rev, a).await
    }
    pub async fn path(&self, p: &Principal) -> AppResult<PathEditor> {
        authorize(p)?;
        self.store.path().await
    }
    pub async fn order(
        &self,
        p: &Principal,
        a: AuditActor,
        ids: Vec<Uuid>,
        rev: i64,
    ) -> AppResult<PathEditor> {
        authorize(p)?;
        revision(rev)?;
        if ids.iter().collect::<HashSet<_>>().len() != ids.len() {
            return Err(AppError::Invalid("INVALID_PATH_ORDER"));
        }
        self.store.order(ids, rev, a).await
    }
}
pub use crate::modules::identity::application::{
    authorize_content as authorize, content_actor as actor,
};

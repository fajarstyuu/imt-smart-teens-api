//! Repository contracts obtained inside one caller-owned transaction scope.
//! Adapters only map/load/write; prerequisite, scoring and idempotency decisions stay in services.
pub use crate::modules::learning::domain::PublishedPath;
use crate::{
    modules::{
        anthropometry::domain::Measurement, assessment::domain::*,
        identity::application::NewSession, learning::domain::*,
    },
    shared::error::AppResult,
};
use async_trait::async_trait;
use chrono::NaiveDate;
use serde_json::Value;
use uuid::Uuid;
pub struct CachedOperation {
    pub payload_hash: Vec<u8>,
    pub status: u16,
    pub body: Value,
}
#[async_trait]
pub trait IdentityRepository: Send {
    async fn lock_school(&mut self, id: Uuid) -> AppResult<Option<(String, bool)>>;
    async fn insert_student(&mut self, id: Uuid, name: &str) -> AppResult<()>;
    async fn insert_session(
        &mut self,
        s: &NewSession,
        old: Option<&[u8]>,
        sibling: Option<&[u8]>,
    ) -> AppResult<()>;
}
#[async_trait]
pub trait LearningRepository: Send {
    async fn published_path(&mut self) -> AppResult<Option<PublishedPath>>;
    async fn insert_enrollment(
        &mut self,
        scope: StudentScope,
        school: Uuid,
        label: &str,
        date: NaiveDate,
        path: Uuid,
    ) -> AppResult<()>;
    /// Locks enrollment BEFORE any attempt for transitions. All queries validate both owner IDs.
    async fn journey(&mut self, scope: StudentScope) -> AppResult<Journey>;
    async fn lesson_view(&mut self, scope: StudentScope, lesson: &Lesson) -> AppResult<LessonView>;
    async fn open_lesson(
        &mut self,
        scope: StudentScope,
        version: Uuid,
        section: Option<Uuid>,
    ) -> AppResult<()>;
    async fn complete_lesson(&mut self, scope: StudentScope, version: Uuid) -> AppResult<()>;
    async fn complete_enrollment(&mut self, scope: StudentScope) -> AppResult<()>;
}
#[async_trait]
pub trait AssessmentRepository: Send {
    async fn pin_snapshots(&mut self, enrollment: Uuid, path: &PublishedPath) -> AppResult<()>;
    async fn create_attempt(&mut self, enrollment: Uuid, typ: AssessmentType)
    -> AppResult<Attempt>;
    /// Autosave takes only the attempt lock. It NEVER acquires enrollment afterwards.
    async fn lock_attempt(&mut self, scope: StudentScope, id: Uuid) -> AppResult<Attempt>;
    async fn questions(&mut self, snapshot: Uuid) -> AppResult<Vec<Question>>;
    async fn answers(&mut self, attempt: Uuid) -> AppResult<Vec<Answer>>;
    async fn write_answer(
        &mut self,
        attempt: &Attempt,
        q: &Question,
        ids: &[Uuid],
        revision: i64,
    ) -> AppResult<Answer>;
    async fn score_answer(
        &mut self,
        attempt: Uuid,
        q: Uuid,
        earned: i64,
        correct: bool,
    ) -> AppResult<()>;
    async fn submit_attempt(
        &mut self,
        attempt: Uuid,
        earned: i64,
        max: i64,
        percent: i32,
    ) -> AppResult<()>;
}
#[async_trait]
pub trait MeasurementRepository: Send {
    async fn insert_measurement(
        &mut self,
        scope: StudentScope,
        weight: &str,
        height: &str,
        bmi: &str,
        display: &str,
    ) -> AppResult<Measurement>;
    async fn measurement_history(
        &mut self,
        scope: StudentScope,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<Measurement>, i64)>;
}
#[async_trait]
pub trait OperationRepository: Send {
    /// Serializes key reservation in this transaction, including concurrent first requests.
    async fn operation(
        &mut self,
        scope: &[u8],
        operation: &str,
        key: &str,
    ) -> AppResult<Option<CachedOperation>>;
    async fn remember(
        &mut self,
        scope: &[u8],
        operation: &str,
        key: &str,
        value: CachedOperation,
    ) -> AppResult<()>;
    async fn audit(&mut self, action: &str, resource: Uuid, request_id: &str) -> AppResult<()>;
}
#[async_trait]
pub trait StudentTransaction:
    IdentityRepository
    + LearningRepository
    + AssessmentRepository
    + MeasurementRepository
    + OperationRepository
{
    async fn commit(self: Box<Self>) -> AppResult<()>;
}
#[async_trait]
pub trait StudentUnitOfWork: Send + Sync {
    async fn begin(&self) -> AppResult<Box<dyn StudentTransaction>>;
}

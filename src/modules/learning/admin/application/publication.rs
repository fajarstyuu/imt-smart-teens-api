//! Learning-path publication coordinates assessment admission through inward transaction ports.
use super::*;
use crate::modules::assessment::admin::application::{BankKind, PublishedBankReader};

/// Persisted lesson facts are not filtered for availability by adapters. Core admits the
/// latest explicit publication, including a dirty root, but never an archived lesson.
pub struct PublishedLesson {
    pub lesson_id: Uuid,
    pub state: ContentState,
    pub version: Option<PublishedLessonVersion>,
}
pub struct PublishedLessonVersion {
    pub id: Uuid,
    pub draft: LessonDraft,
}
impl PublishedLesson {
    fn admit(self, expected_id: Uuid) -> AppResult<Uuid> {
        if self.lesson_id != expected_id || self.state == ContentState::Archived {
            return Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"));
        }
        let version = self
            .version
            .ok_or(AppError::Conflict("CONTENT_NOT_AVAILABLE"))?;
        version.draft.validate(true)?;
        Ok(version.id)
    }
}
/// Fully admitted write intent. Adapters construct/seal children, persist revisions and
/// audit, but do not choose the bank/lesson publications or their admission outcome.
pub struct PathPublication {
    pub id: Uuid,
    pub path_id: Uuid,
    pub version: i32,
    pub revision: i64,
    pub bank_versions: [Uuid; 2],
    pub lesson_versions: Vec<Uuid>,
}
#[async_trait]
pub trait PathPublicationTransaction: PublishedBankReader {
    async fn lock_path(&mut self) -> AppResult<PathEditor>;
    // Legacy publication-only adapters need not implement draft release; concrete release
    // adapters provide all four operations under their existing transaction.
    async fn lesson_draft(&mut self, _id: Uuid) -> AppResult<LessonEditor> {
        Err(AppError::Internal)
    }
    async fn bank_drafts(
        &mut self,
        _id: Uuid,
    ) -> AppResult<(
        crate::modules::assessment::admin::application::BankEditor,
        Vec<crate::modules::assessment::admin::application::QuestionEditor>,
    )> {
        Err(AppError::Internal)
    }
    async fn publish_draft_lesson(
        &mut self,
        _id: Uuid,
        _revision: i64,
        _actor: AuditActor,
    ) -> AppResult<()> {
        Err(AppError::Internal)
    }
    async fn publish_draft_bank(
        &mut self,
        _id: Uuid,
        _revision: i64,
        _actor: AuditActor,
    ) -> AppResult<()> {
        Err(AppError::Internal)
    }
    async fn lock_lesson_roots(&mut self, sorted_ids: &[Uuid]) -> AppResult<()>;
    async fn published_lesson(&mut self, id: Uuid) -> AppResult<Option<PublishedLesson>>;
    async fn persist_manifest(
        &mut self,
        publication: PathPublication,
        actor: AuditActor,
    ) -> AppResult<PathEditor>;
    /// Consuming commit prevents accidental reuse. Dropping on any failure rolls back all writes.
    async fn commit(self: Box<Self>) -> AppResult<()>;
}
/// One transaction from root locks through admission, immutable construction and audit.
/// No separate bank transaction can race with admission. All policy precedes manifest writes.
pub async fn publish_path_in_transaction(
    mut tx: Box<dyn PathPublicationTransaction>,
    rev: i64,
    actor: AuditActor,
) -> AppResult<PathEditor> {
    let p = tx.lock_path().await?;
    check_revision(p.revision, rev)?;
    if p.lesson_ids.is_empty() {
        return Err(AppError::Invalid("EMPTY_LEARNING_PATH"));
    }
    if p.lesson_ids.iter().collect::<HashSet<_>>().len() != p.lesson_ids.len() {
        return Err(AppError::Invalid("INVALID_PATH_ORDER"));
    }
    // Every CMS writer observes path -> sorted banks -> sorted lesson roots.
    let mut banks = [p.paired_bank_id, p.quiz_bank_id];
    banks.sort();
    tx.lock_bank_roots(&banks).await?;
    let mut bank_versions = [Uuid::nil(); 2];
    for (index, (id, kind)) in [
        (p.paired_bank_id, BankKind::Paired),
        (p.quiz_bank_id, BankKind::Quiz),
    ]
    .into_iter()
    .enumerate()
    {
        let bank = tx
            .published_bank(id)
            .await?
            .ok_or(AppError::Conflict("CONTENT_NOT_AVAILABLE"))?;
        bank_versions[index] = bank.admit(id, kind)?;
    }
    let mut sorted = p.lesson_ids.clone();
    sorted.sort();
    tx.lock_lesson_roots(&sorted).await?;
    let mut lesson_versions = vec![];
    for id in &p.lesson_ids {
        lesson_versions.push(
            tx.published_lesson(*id)
                .await?
                .ok_or(AppError::Conflict("CONTENT_NOT_AVAILABLE"))?
                .admit(*id)?,
        );
    }
    let out = tx
        .persist_manifest(
            PathPublication {
                id: Uuid::new_v4(),
                path_id: p.id,
                version: p.versions.first().map_or(1, |v| v.version + 1),
                revision: rev + 1,
                bank_versions,
                lesson_versions,
            },
            actor,
        )
        .await?;
    tx.commit().await?;
    Ok(out)
}

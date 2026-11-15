//! Assessment-owned admission contract for other application workflows.
//! Facts are untrusted persisted content, not proof of admission. The caller must validate
//! them while holding the bank root locks until its enclosing publication commits.
use super::{ContentState, QuestionDraft};
use crate::{
    modules::assessment::domain::validate_score_budget,
    shared::error::{AppError, AppResult},
};
use async_trait::async_trait;
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BankKind {
    Paired,
    Quiz,
}
impl BankKind {
    pub fn parse(value: &str) -> AppResult<Self> {
        match value {
            "paired" => Ok(Self::Paired),
            "quiz" => Ok(Self::Quiz),
            _ => Err(AppError::Invalid("INVALID_PUBLISHED_BANK")),
        }
    }
}
#[derive(Clone, Debug)]
pub struct PublishedQuestion {
    pub bank_id: Uuid,
    pub state: ContentState,
    pub position: i32,
    pub scoring_rule: String,
    pub max_points_units: i64,
    pub draft: QuestionDraft,
}
#[derive(Clone, Debug)]
pub struct PublishedBank {
    pub id: Uuid,
    pub kind: BankKind,
    pub state: ContentState,
    pub version_id: Option<Uuid>,
    pub version_bank_id: Option<Uuid>,
    pub questions: Vec<PublishedQuestion>,
}
impl PublishedBank {
    /// Dirty roots may still supply their last explicit publication; archive, foreign
    /// identities, malformed immutable questions and aggregate overflow cannot be admitted.
    pub fn admit(&self, expected_id: Uuid, expected_kind: BankKind) -> AppResult<Uuid> {
        if self.id != expected_id
            || self.kind != expected_kind
            || self.state == ContentState::Archived
            || self.version_bank_id != Some(expected_id)
        {
            return Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"));
        }
        let version = self
            .version_id
            .ok_or(AppError::Conflict("CONTENT_NOT_AVAILABLE"))?;
        let mut positions = HashSet::new();
        for q in &self.questions {
            if q.bank_id != expected_id
                || q.state == ContentState::Archived
                || q.scoring_rule != "exact_set_v1"
                || q.position < 1
                || !positions.insert(q.position)
            {
                return Err(AppError::Invalid("INVALID_PUBLISHED_BANK"));
            }
            q.draft.validate()?;
        }
        validate_score_budget(self.questions.iter().map(|q| q.max_points_units))?;
        Ok(version)
    }
}
/// Transaction-scoped port. Implementations use the enclosing workflow's transaction,
/// never a second pooled connection. Lock order is supplied by the coordinating core.
#[async_trait]
pub trait PublishedBankReader: Send {
    async fn lock_bank_roots(&mut self, sorted_ids: &[Uuid]) -> AppResult<()>;
    async fn published_bank(&mut self, id: Uuid) -> AppResult<Option<PublishedBank>>;
}

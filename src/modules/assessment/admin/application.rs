//! Typed admin commands and question publication validation. No student DTO reuse.
use crate::{
    modules::{
        identity::application::authorize_content as authorize,
        identity::{application::AuditActor, domain::Principal},
    },
    shared::{
        editing::{ContentQuery, ContentState, Page, Version, revision, text},
        error::{AppError, AppResult},
    },
};
use async_trait::async_trait;
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestionType {
    Single,
    Multiple,
}
impl QuestionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Multiple => "multiple",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "single" => Ok(Self::Single),
            "multiple" => Ok(Self::Multiple),
            _ => Err(AppError::Invalid("INVALID_QUESTION_TYPE")),
        }
    }
}
#[derive(Clone, Debug)]
pub struct OptionDraft {
    pub id: Uuid,
    pub text: String,
}
#[derive(Clone, Debug)]
pub struct QuestionDraft {
    pub text: String,
    pub question_type: QuestionType,
    pub options: Vec<OptionDraft>,
    pub correct_option_ids: Vec<Uuid>,
    pub required: bool,
}
impl QuestionDraft {
    /// Every admitted draft is structurally publishable; bank publication revalidates storage.
    pub fn validate(&self) -> AppResult<()> {
        text(&self.text, 10000, false)?;
        if !(2..=100).contains(&self.options.len()) || !self.required {
            return Err(AppError::Invalid("INVALID_QUESTION"));
        }
        let mut ids = HashSet::new();
        let mut labels = HashSet::new();
        for o in &self.options {
            text(&o.text, 2000, false)?;
            if !ids.insert(o.id) || !labels.insert(o.text.trim()) {
                return Err(AppError::Invalid("INVALID_OPTIONS"));
            }
        }
        let keys: HashSet<_> = self.correct_option_ids.iter().copied().collect();
        if keys.is_empty()
            || keys.len() != self.correct_option_ids.len()
            || !keys.is_subset(&ids)
            || (self.question_type == QuestionType::Single && keys.len() != 1)
        {
            return Err(AppError::Invalid("INVALID_ANSWER_KEY"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub struct QuestionEditor {
    pub id: Uuid,
    pub bank_id: Uuid,
    pub state: ContentState,
    pub revision: i64,
    pub position: i32,
    pub draft: QuestionDraft,
    pub versions: Vec<Version>,
}
#[derive(Clone, Debug)]
pub struct BankEditor {
    pub id: Uuid,
    pub kind: String,
    pub state: ContentState,
    pub revision: i64,
    pub versions: Vec<Version>,
}
#[async_trait]
pub trait BankStore: Send + Sync {
    async fn banks(&self) -> AppResult<Vec<BankEditor>>;
    async fn questions(&self, bank: Uuid, q: ContentQuery) -> AppResult<Page<QuestionEditor>>;
    async fn question(&self, bank: Uuid, id: Uuid) -> AppResult<QuestionEditor>;
    async fn create(
        &self,
        bank: Uuid,
        d: QuestionDraft,
        position: i32,
        a: AuditActor,
    ) -> AppResult<QuestionEditor>;
    async fn update(
        &self,
        bank: Uuid,
        id: Uuid,
        d: QuestionDraft,
        position: i32,
        rev: i64,
        a: AuditActor,
    ) -> AppResult<QuestionEditor>;
    async fn archive(
        &self,
        bank: Uuid,
        id: Uuid,
        rev: i64,
        a: AuditActor,
    ) -> AppResult<QuestionEditor>;
    async fn publish(&self, bank: Uuid, rev: i64, a: AuditActor) -> AppResult<BankEditor>;
}
#[derive(Clone)]
pub struct BankCms {
    store: Arc<dyn BankStore>,
}
impl BankCms {
    pub fn new(store: Arc<dyn BankStore>) -> Self {
        Self { store }
    }
    pub async fn banks(&self, p: &Principal) -> AppResult<Vec<BankEditor>> {
        authorize(p)?;
        self.store.banks().await
    }
    pub async fn questions(
        &self,
        p: &Principal,
        bank: Uuid,
        q: ContentQuery,
    ) -> AppResult<Page<QuestionEditor>> {
        authorize(p)?;
        q.validate()?;
        self.store.questions(bank, q).await
    }
    pub async fn question(&self, p: &Principal, bank: Uuid, id: Uuid) -> AppResult<QuestionEditor> {
        authorize(p)?;
        self.store.question(bank, id).await
    }
    pub async fn create(
        &self,
        p: &Principal,
        a: AuditActor,
        bank: Uuid,
        d: QuestionDraft,
        pos: i32,
    ) -> AppResult<QuestionEditor> {
        authorize(p)?;
        d.validate()?;
        position(pos)?;
        self.store.create(bank, d, pos, a).await
    }
    pub async fn update(
        &self,
        p: &Principal,
        a: AuditActor,
        ids: (Uuid, Uuid),
        d: QuestionDraft,
        pos: i32,
        rev: i64,
    ) -> AppResult<QuestionEditor> {
        authorize(p)?;
        d.validate()?;
        position(pos)?;
        revision(rev)?;
        self.store.update(ids.0, ids.1, d, pos, rev, a).await
    }
    pub async fn archive(
        &self,
        p: &Principal,
        a: AuditActor,
        ids: (Uuid, Uuid),
        rev: i64,
    ) -> AppResult<QuestionEditor> {
        authorize(p)?;
        revision(rev)?;
        self.store.archive(ids.0, ids.1, rev, a).await
    }
    pub async fn publish(
        &self,
        p: &Principal,
        a: AuditActor,
        bank: Uuid,
        rev: i64,
    ) -> AppResult<BankEditor> {
        authorize(p)?;
        revision(rev)?;
        self.store.publish(bank, rev, a).await
    }
}
fn position(p: i32) -> AppResult<()> {
    if p < 1 {
        Err(AppError::Invalid("INVALID_POSITION"))
    } else {
        Ok(())
    }
}

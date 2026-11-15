//! Snapshot-owned question models and pure answer/scoring invariants.
use crate::shared::error::{AppError, AppResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentType {
    PreTest,
    Quiz,
    PostTest,
}
impl AssessmentType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreTest => "pre_test",
            Self::Quiz => "quiz",
            Self::PostTest => "post_test",
        }
    }
    pub fn kind(self) -> &'static str {
        if self == Self::Quiz { "quiz" } else { "paired" }
    }
}
#[derive(Clone, Serialize)]
pub struct Attempt {
    pub id: Uuid,
    pub assessment_type: AssessmentType,
    pub attempt_number: i32,
    pub status: String,
    pub snapshot_id: Uuid,
    pub submitted_at: Option<DateTime<Utc>>,
    pub score: Option<i32>,
}
#[derive(Clone, Serialize)]
pub struct Answer {
    pub snapshot_question_id: Uuid,
    pub option_ids: Vec<Uuid>,
    pub revision: i64,
    pub saved_at: DateTime<Utc>,
}
#[derive(Clone, Serialize)]
pub struct SafeOption {
    pub id: Uuid,
    pub text: String,
}
#[derive(Clone, Serialize)]
pub struct SafeQuestion {
    pub id: Uuid,
    pub question_id: Uuid,
    pub question_version_id: Uuid,
    pub position: i32,
    pub text: String,
    #[serde(rename = "type")]
    pub question_type: String,
    pub required: bool,
    pub options: Vec<SafeOption>,
}
/// Private scoring data is never serializable. Mapping to SafeQuestion is explicit.
pub struct Question {
    pub safe: SafeQuestion,
    pub correct_option_ids: Vec<Uuid>,
    pub max_units: i64,
}
impl Question {
    pub fn validate_answer(&self, ids: &[Uuid]) -> AppResult<()> {
        let unique = ids.iter().collect::<std::collections::HashSet<_>>();
        if unique.len() != ids.len()
            || (self.safe.question_type == "single" && ids.len() > 1)
            || ids
                .iter()
                .any(|id| !self.safe.options.iter().any(|o| o.id == *id))
        {
            return Err(AppError::Invalid("INVALID_ANSWER"));
        }
        Ok(())
    }
    pub fn score(&self, ids: &[Uuid]) -> AppResult<(i64, bool)> {
        self.validate_answer(ids)?;
        if self.safe.required && ids.is_empty() {
            return Err(AppError::Invalid("REQUIRED_ANSWERS_MISSING"));
        }
        let correct = !ids.is_empty()
            && ids.len() == self.correct_option_ids.len()
            && ids.iter().all(|id| self.correct_option_ids.contains(id));
        Ok((if correct { self.max_units } else { 0 }, correct))
    }
}
/// Maximum aggregate representable by persisted NUMERIC(12,4) earned/max points.
pub const MAX_BANK_SCORE_UNITS: i64 = 999_999_999_999;

/// Validate complete bank/snapshot weights before admission or any scoring writes.
/// Task3 publication must call this same policy. Units are exact ten-thousandths;
/// empty, nonpositive, overflowing or unrepresentable totals are unavailable content.
pub fn validate_score_budget(weights: impl IntoIterator<Item = i64>) -> AppResult<i64> {
    let mut total = 0i64;
    for weight in weights {
        if weight <= 0 {
            return Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"));
        }
        total = total
            .checked_add(weight)
            .filter(|sum| *sum <= MAX_BANK_SCORE_UNITS)
            .ok_or(AppError::Conflict("CONTENT_NOT_AVAILABLE"))?;
    }
    if total == 0 {
        return Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"));
    }
    Ok(total)
}
#[derive(Serialize)]
pub struct AttemptView {
    #[serde(flatten)]
    pub attempt: Attempt,
    pub questions: Vec<SafeQuestion>,
    pub answers: Vec<Answer>,
}

//! Domain model for the single-choice mastery check embedded in each lesson version.
//! Unlike the pre/post-test banks, this contract deliberately has two stable ordered options.
use crate::shared::error::{AppError, AppResult};
use std::collections::HashSet;
use uuid::Uuid;

/// A stable option identity and its learner-facing label.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LessonQuizOption {
    pub id: Uuid,
    pub text: String,
}

/// Editable versioned question content. Correctness is never serialized to student DTOs.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LessonQuizDraft {
    pub text: String,
    pub options: Vec<LessonQuizOption>,
    pub correct_option_id: Option<Uuid>,
}

impl LessonQuizDraft {
    /// Validate stable A/B identity in both draft and publish modes; only text/key completeness
    /// is deferred while drafting. The caller decides whether invalid drafts block publication.
    pub fn validate(&self, publishing: bool) -> AppResult<()> {
        if self.options.len() != 2 {
            return Err(AppError::Invalid("INVALID_LESSON_QUIZ_OPTIONS"));
        }
        let ids: HashSet<_> = self.options.iter().map(|option| option.id).collect();
        if ids.len() != 2 || ids.contains(&Uuid::nil()) {
            return Err(AppError::Invalid("INVALID_LESSON_QUIZ_OPTIONS"));
        }
        if self
            .correct_option_id
            .is_some_and(|correct| !ids.contains(&correct))
        {
            return Err(AppError::Invalid("INVALID_LESSON_QUIZ_KEY"));
        }
        if publishing
            && (self.text.trim().is_empty()
                || self
                    .options
                    .iter()
                    .any(|option| option.text.trim().is_empty())
                || self.correct_option_id.is_none())
        {
            return Err(AppError::Invalid("INCOMPLETE_LESSON_QUIZ"));
        }
        if self.text.len() > 10_000 || self.options.iter().any(|option| option.text.len() > 2_000) {
            return Err(AppError::Invalid("INVALID_LESSON_QUIZ_TEXT"));
        }
        Ok(())
    }
}

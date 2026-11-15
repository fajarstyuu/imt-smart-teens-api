//! Durable enrollment projections and prerequisite policy, independent of HTTP/SQL.
use crate::{
    modules::{
        anthropometry::domain::Measurement,
        assessment::domain::{AssessmentType, Attempt},
    },
    shared::error::{AppError, AppResult},
};
use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;
/// Published manifest facts; availability policy belongs here, never inside a SQL expression.
pub struct PublishedPath {
    pub id: Uuid,
    pub paired_bank: Uuid,
    pub quiz_bank: Uuid,
    pub paired_kind: String,
    pub quiz_kind: String,
    pub lesson_count: i64,
    pub paired_question_count: i64,
    pub quiz_question_count: i64,
    pub paired_weights: Vec<i64>,
    pub quiz_weights: Vec<i64>,
}
impl PublishedPath {
    /// Reject incomplete/mismatched manifests without creating any enrollment effects.
    pub fn require_available(&self) -> AppResult<()> {
        crate::modules::assessment::domain::validate_score_budget(
            self.paired_weights.iter().copied(),
        )?;
        crate::modules::assessment::domain::validate_score_budget(
            self.quiz_weights.iter().copied(),
        )?;
        if self.lesson_count > 0
            && self.paired_question_count > 0
            && self.quiz_question_count > 0
            && self.paired_kind == "paired"
            && self.quiz_kind == "quiz"
        {
            Ok(())
        } else {
            Err(AppError::Conflict("CONTENT_NOT_AVAILABLE"))
        }
    }
}
#[derive(Clone, Copy)]
pub struct StudentScope {
    pub student_id: Uuid,
    pub enrollment_id: Uuid,
}
#[derive(Clone)]
pub struct Student {
    pub id: Uuid,
    pub name: String,
}
#[derive(Clone)]
pub struct Enrollment {
    pub id: Uuid,
    pub school_id: Uuid,
    pub school_name_at_enrollment: String,
    pub form_date: NaiveDate,
    pub path_version_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
#[derive(Clone)]
pub struct Lesson {
    pub lesson_id: Uuid,
    pub lesson_version_id: Uuid,
    pub title: String,
    pub position: i32,
    pub opened_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}
#[derive(Clone)]
pub struct Section {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub required: bool,
    pub position: i32,
}
#[derive(Clone)]
pub struct LessonView {
    pub lesson: Lesson,
    pub description: String,
    pub sections: Vec<Section>,
    pub visited_section_ids: Vec<Uuid>,
}
/// Ordered journey stages; these are business states, not response field names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    PreTest,
    Lessons,
    Quiz,
    Calculator,
    PostTest,
    Completed,
}
impl Stage {
    pub const ALL: [Self; 6] = [
        Self::PreTest,
        Self::Lessons,
        Self::Quiz,
        Self::Calculator,
        Self::PostTest,
        Self::Completed,
    ];
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressStatus {
    Locked,
    NotStarted,
    InProgress,
    Completed,
}
#[derive(Clone)]
pub struct StageProgress {
    pub stage: Stage,
    pub status: ProgressStatus,
    pub prerequisite_missing: bool,
}
#[derive(Clone)]
pub struct LessonProgress {
    pub lesson: Lesson,
    pub status: ProgressStatus,
}
/// Safe, typed prerequisite projection shared by reads and committed mutation results.
#[derive(Clone)]
pub struct Progress {
    pub enrollment_id: Uuid,
    pub path_version_id: Uuid,
    pub current_stage: Stage,
    pub stages: Vec<StageProgress>,
    pub lessons: Vec<LessonProgress>,
    pub active_attempt_ids: Vec<(AssessmentType, Uuid)>,
    pub latest_measurement_id: Option<Uuid>,
}
/// A single consistent database projection; stage flags are derived, never persisted twice.
pub struct Journey {
    pub student: Student,
    pub enrollment: Enrollment,
    pub lessons: Vec<Lesson>,
    pub attempts: Vec<Attempt>,
    pub latest_measurement: Option<Measurement>,
}
impl Journey {
    pub fn submitted(&self, typ: AssessmentType) -> bool {
        self.attempts
            .iter()
            .any(|a| a.assessment_type == typ && a.status == "submitted")
    }
    pub fn all_lessons_complete(&self) -> bool {
        !self.lessons.is_empty() && self.lessons.iter().all(|l| l.completed_at.is_some())
    }
    pub fn require_attempt(&self, typ: AssessmentType) -> AppResult<()> {
        let allowed = match typ {
            AssessmentType::PreTest => true,
            AssessmentType::Quiz => {
                self.submitted(AssessmentType::PreTest) && self.all_lessons_complete()
            }
            AssessmentType::PostTest => {
                self.submitted(AssessmentType::Quiz) && self.latest_measurement.is_some()
            }
        };
        if allowed {
            Ok(())
        } else {
            Err(AppError::Conflict("LEARNING_PREREQUISITE_NOT_MET"))
        }
    }
    pub fn lesson(&self, id: Uuid) -> AppResult<&Lesson> {
        let index = self
            .lessons
            .iter()
            .position(|l| l.lesson_id == id)
            .ok_or(AppError::NotFound)?;
        if !self.submitted(AssessmentType::PreTest)
            || self.lessons[..index]
                .iter()
                .any(|l| l.completed_at.is_none())
        {
            return Err(AppError::Conflict("LEARNING_PREREQUISITE_NOT_MET"));
        }
        Ok(&self.lessons[index])
    }
    pub fn require_measurement(&self) -> AppResult<()> {
        if self.submitted(AssessmentType::Quiz) && !self.submitted(AssessmentType::PostTest) {
            Ok(())
        } else {
            Err(AppError::Conflict("LEARNING_PREREQUISITE_NOT_MET"))
        }
    }
    pub fn current_stage(&self) -> Stage {
        if !self.submitted(AssessmentType::PreTest) {
            Stage::PreTest
        } else if !self.all_lessons_complete() {
            Stage::Lessons
        } else if !self.submitted(AssessmentType::Quiz) {
            Stage::Quiz
        } else if self.latest_measurement.is_none() {
            Stage::Calculator
        } else if !self.submitted(AssessmentType::PostTest) {
            Stage::PostTest
        } else {
            Stage::Completed
        }
    }
    pub fn progress(&self) -> Progress {
        let current = self.current_stage();
        let index = Stage::ALL.iter().position(|s| *s == current).unwrap_or(0);
        let stages = Stage::ALL
            .into_iter()
            .enumerate()
            .map(|(i, stage)| StageProgress {
                stage,
                status: if i < index || current == Stage::Completed {
                    ProgressStatus::Completed
                } else if i == index {
                    ProgressStatus::InProgress
                } else {
                    ProgressStatus::Locked
                },
                prerequisite_missing: i > index,
            })
            .collect();
        let lessons = self
            .lessons
            .iter()
            .map(|l| LessonProgress {
                lesson: l.clone(),
                status: if self.lesson(l.lesson_id).is_err() {
                    ProgressStatus::Locked
                } else if l.completed_at.is_some() {
                    ProgressStatus::Completed
                } else if l.opened_at.is_some() {
                    ProgressStatus::InProgress
                } else {
                    ProgressStatus::NotStarted
                },
            })
            .collect();
        Progress {
            enrollment_id: self.enrollment.id,
            path_version_id: self.enrollment.path_version_id,
            current_stage: current,
            stages,
            lessons,
            active_attempt_ids: self
                .attempts
                .iter()
                .filter(|a| a.status == "in_progress")
                .map(|a| (a.assessment_type, a.id))
                .collect(),
            latest_measurement_id: self.latest_measurement.as_ref().map(|m| m.id),
        }
    }
    pub fn visible_score(&self, a: &Attempt) -> Option<i32> {
        if a.assessment_type == AssessmentType::Quiz || self.submitted(AssessmentType::PostTest) {
            a.score
        } else {
            None
        }
    }
}

//! Typed reporting ports, filter validation and state semantics independent of transport/SQL.
use crate::shared::editing::Page;
use crate::{
    modules::{
        anthropometry::domain::Measurement,
        assessment::domain::AssessmentType,
        identity::domain::Principal,
        learning::domain::{Enrollment, Lesson, Stage, Student},
    },
    shared::error::{AppError, AppResult},
};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone, Copy)]
pub struct Window {
    pub limit: i64,
    pub offset: i64,
}
impl Window {
    pub fn validate(self) -> AppResult<()> {
        if !(1..=100).contains(&self.limit) || self.offset < 0 {
            Err(AppError::Invalid("INVALID_PAGINATION"))
        } else {
            Ok(())
        }
    }
}
#[derive(Default)]
pub struct Filters {
    pub school_id: Option<Uuid>,
    pub form_date_from: Option<NaiveDate>,
    pub form_date_to: Option<NaiveDate>,
    pub submitted_from: Option<DateTime<Utc>>,
    pub submitted_to: Option<DateTime<Utc>>,
    pub stage: Option<Stage>,
    pub assessment_type: Option<AssessmentType>,
}
impl Filters {
    pub fn validate(&self) -> AppResult<()> {
        if self
            .form_date_from
            .zip(self.form_date_to)
            .is_some_and(|(a, b)| a > b)
            || self
                .submitted_from
                .zip(self.submitted_to)
                .is_some_and(|(a, b)| a > b)
        {
            Err(AppError::Invalid("INVALID_REPORT_RANGE"))
        } else {
            Ok(())
        }
    }
}
pub fn parse_stage(s: &str) -> AppResult<Stage> {
    match s {
        "pre_test" => Ok(Stage::PreTest),
        "lessons" => Ok(Stage::Lessons),
        "quiz" => Ok(Stage::Quiz),
        "calculator" => Ok(Stage::Calculator),
        "post_test" => Ok(Stage::PostTest),
        "completed" => Ok(Stage::Completed),
        _ => Err(AppError::Invalid("INVALID_REPORT_STAGE")),
    }
}
pub fn stage_name(s: Stage) -> &'static str {
    match s {
        Stage::PreTest => "pre_test",
        Stage::Lessons => "lessons",
        Stage::Quiz => "quiz",
        Stage::Calculator => "calculator",
        Stage::PostTest => "post_test",
        Stage::Completed => "completed",
    }
}
pub fn parse_type(s: &str) -> AppResult<AssessmentType> {
    match s {
        "pre_test" => Ok(AssessmentType::PreTest),
        "quiz" => Ok(AssessmentType::Quiz),
        "post_test" => Ok(AssessmentType::PostTest),
        _ => Err(AppError::Invalid("INVALID_ASSESSMENT_TYPE")),
    }
}
/// Points are exact ten-thousandths, never JSON/binary floating point in the core.
#[derive(Clone, Copy)]
pub struct Points(i64);
impl Points {
    pub fn from_units(units: i64) -> AppResult<Self> {
        if !(0..=crate::modules::assessment::domain::MAX_BANK_SCORE_UNITS).contains(&units) {
            return Err(AppError::Internal);
        }
        Ok(Self(units))
    }
    pub fn decimal(self) -> String {
        format!("{}.{:04}", self.0 / 10000, self.0 % 10000)
    }
}
pub struct StudentSummary {
    pub student: Student,
    pub enrollment: Enrollment,
    pub stage: Stage,
    pub latest_submitted_at: Option<DateTime<Utc>>,
}
pub struct AttemptSummary {
    pub id: Uuid,
    pub snapshot_id: Uuid,
    pub assessment_type: AssessmentType,
    pub attempt_number: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub earned_points: Option<Points>,
    pub max_points: Option<Points>,
    pub score_percent: Option<i32>,
}
pub struct StudentResults {
    pub summary: StudentSummary,
    pub attempts: Vec<AttemptSummary>,
    pub lessons: Vec<Lesson>,
    pub measurements: Vec<Measurement>,
}
#[derive(Clone, Copy)]
pub enum AnswerState {
    NotStarted,
    Unanswered,
    Answered,
    Incorrect,
    Correct,
}
impl AnswerState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::Unanswered => "unanswered",
            Self::Answered => "answered",
            Self::Incorrect => "incorrect",
            Self::Correct => "correct",
        }
    }
}
pub struct ReportOption {
    pub id: Uuid,
    pub text: String,
}
pub struct QuestionAnswer {
    pub student_id: Uuid,
    pub student_name: String,
    pub enrollment_id: Uuid,
    pub school_id: Uuid,
    pub school_name_at_enrollment: String,
    pub bank_id: Uuid,
    pub bank_version_id: Uuid,
    pub snapshot_id: Uuid,
    pub snapshot_question_id: Uuid,
    pub question_id: Uuid,
    pub question_version_id: Uuid,
    pub position: i32,
    pub text: String,
    pub question_type: String,
    pub assessment_type: AssessmentType,
    pub attempt_id: Option<Uuid>,
    pub attempt_number: Option<i32>,
    pub attempt_status: Option<String>,
    pub answer_revision: Option<i64>,
    pub saved_at: Option<DateTime<Utc>>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub options: Vec<ReportOption>,
    pub selected_options: Vec<ReportOption>,
    pub correctness: Option<bool>,
    pub earned_points: Option<Points>,
    pub max_points: Points,
}
impl QuestionAnswer {
    pub fn state(&self) -> AnswerState {
        if self.answer_revision.is_none() {
            AnswerState::NotStarted
        } else if self.selected_options.is_empty() {
            AnswerState::Unanswered
        } else if self.submitted_at.is_none() {
            AnswerState::Answered
        } else if self.correctness == Some(true) {
            AnswerState::Correct
        } else {
            AnswerState::Incorrect
        }
    }
}
pub struct QuestionSelection {
    pub version_id: Uuid,
    pub question_id: Option<Uuid>,
    pub bank_id: Option<Uuid>,
}
/// SQL adapters receive admin identity, not an untrusted school list. Every root query
/// joins live grants BEFORE counting/pagination or selecting a detail candidate.
#[async_trait]
pub trait ReportReader: Send + Sync {
    async fn students(
        &self,
        admin: Uuid,
        filters: Filters,
        page: Window,
    ) -> AppResult<Page<StudentSummary>>;
    async fn results(
        &self,
        admin: Uuid,
        student: Uuid,
        enrollment: Option<Uuid>,
    ) -> AppResult<StudentResults>;
    async fn attempt_answers(
        &self,
        admin: Uuid,
        attempt: Uuid,
        page: Window,
    ) -> AppResult<Page<QuestionAnswer>>;
    async fn question_answers(
        &self,
        admin: Uuid,
        selection: QuestionSelection,
        filters: Filters,
        page: Window,
    ) -> AppResult<Page<QuestionAnswer>>;
}
#[derive(Clone)]
pub struct Reports {
    reader: Arc<dyn ReportReader>,
}
impl Reports {
    pub fn new(reader: Arc<dyn ReportReader>) -> Self {
        Self { reader }
    }
    fn scope(p: &Principal, f: &Filters) -> AppResult<Uuid> {
        let a = p.admin()?;
        if let Some(s) = f.school_id {
            a.require_school_scope(s)?;
        }
        Ok(a.id)
    }
    pub async fn students(
        &self,
        p: &Principal,
        f: Filters,
        w: Window,
    ) -> AppResult<Page<StudentSummary>> {
        f.validate()?;
        w.validate()?;
        self.reader.students(Self::scope(p, &f)?, f, w).await
    }
    pub async fn results(
        &self,
        p: &Principal,
        student: Uuid,
        enrollment: Option<Uuid>,
    ) -> AppResult<StudentResults> {
        self.reader
            .results(p.admin()?.id, student, enrollment)
            .await
    }
    pub async fn attempt_answers(
        &self,
        p: &Principal,
        attempt: Uuid,
        w: Window,
    ) -> AppResult<Page<QuestionAnswer>> {
        w.validate()?;
        self.reader.attempt_answers(p.admin()?.id, attempt, w).await
    }
    pub async fn question_answers(
        &self,
        p: &Principal,
        version: Option<Uuid>,
        question_id: Option<Uuid>,
        bank_id: Option<Uuid>,
        f: Filters,
        w: Window,
    ) -> AppResult<Page<QuestionAnswer>> {
        let version_id = version.ok_or(AppError::Invalid("QUESTION_VERSION_REQUIRED"))?;
        f.validate()?;
        w.validate()?;
        self.reader
            .question_answers(
                Self::scope(p, &f)?,
                QuestionSelection {
                    version_id,
                    question_id,
                    bank_id,
                },
                f,
                w,
            )
            .await
    }
}

//! Explicit student-safe HTTP allowlists. Core/persistence structs are never serialized directly.
use crate::modules::{
    anthropometry::domain::Measurement,
    assessment::domain::*,
    learning::{application::results::*, domain::*},
};
use crate::shared::error::{AppError, AppResult};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_json::Number;
use uuid::Uuid;

pub fn assessment_type(t: AssessmentType) -> &'static str {
    t.as_str()
}
pub fn stage(s: Stage) -> &'static str {
    match s {
        Stage::PreTest => "pre_test",
        Stage::Lessons => "lessons",
        Stage::Quiz => "quiz",
        Stage::Calculator => "calculator",
        Stage::PostTest => "post_test",
        Stage::Completed => "completed",
    }
}
pub fn status(s: ProgressStatus) -> &'static str {
    match s {
        ProgressStatus::Locked => "locked",
        ProgressStatus::NotStarted => "not_started",
        ProgressStatus::InProgress => "in_progress",
        ProgressStatus::Completed => "completed",
    }
}
#[derive(Serialize)]
pub struct StudentDto {
    id: Uuid,
    name: String,
}
impl From<Student> for StudentDto {
    fn from(s: Student) -> Self {
        Self {
            id: s.id,
            name: s.name,
        }
    }
}
#[derive(Serialize)]
pub struct EnrollmentDto {
    id: Uuid,
    school_id: Uuid,
    school_name_at_enrollment: String,
    form_date: NaiveDate,
    path_version_id: Uuid,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}
impl From<Enrollment> for EnrollmentDto {
    fn from(e: Enrollment) -> Self {
        Self {
            id: e.id,
            school_id: e.school_id,
            school_name_at_enrollment: e.school_name_at_enrollment,
            form_date: e.form_date,
            path_version_id: e.path_version_id,
            created_at: e.created_at,
            completed_at: e.completed_at,
        }
    }
}
#[derive(Serialize)]
pub struct ProfileDto {
    pub student: StudentDto,
    pub enrollment: EnrollmentDto,
}
impl From<StudentProfile> for ProfileDto {
    fn from(p: StudentProfile) -> Self {
        Self {
            student: p.student.into(),
            enrollment: p.enrollment.into(),
        }
    }
}
#[derive(Serialize)]
pub struct LessonDto {
    lesson_id: Uuid,
    lesson_version_id: Uuid,
    title: String,
    position: i32,
    opened_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}
impl From<Lesson> for LessonDto {
    fn from(l: Lesson) -> Self {
        Self {
            lesson_id: l.lesson_id,
            lesson_version_id: l.lesson_version_id,
            title: l.title,
            position: l.position,
            opened_at: l.opened_at,
            completed_at: l.completed_at,
        }
    }
}
#[derive(Serialize)]
pub struct StageDto {
    id: &'static str,
    status: &'static str,
    locked_reason: Option<&'static str>,
}
#[derive(Serialize)]
pub struct LessonProgressDto {
    #[serde(flatten)]
    lesson: LessonDto,
    status: &'static str,
}
#[derive(Serialize)]
pub struct ProgressDto {
    enrollment_id: Uuid,
    path_version_id: Uuid,
    current_stage: &'static str,
    stages: Vec<StageDto>,
    lessons: Vec<LessonProgressDto>,
    active_attempt_ids: std::collections::BTreeMap<&'static str, Uuid>,
    latest_measurement_id: Option<Uuid>,
}
impl From<Progress> for ProgressDto {
    fn from(p: Progress) -> Self {
        Self {
            enrollment_id: p.enrollment_id,
            path_version_id: p.path_version_id,
            current_stage: stage(p.current_stage),
            stages: p
                .stages
                .into_iter()
                .map(|s| StageDto {
                    id: stage(s.stage),
                    status: status(s.status),
                    locked_reason: s
                        .prerequisite_missing
                        .then_some("LEARNING_PREREQUISITE_NOT_MET"),
                })
                .collect(),
            lessons: p
                .lessons
                .into_iter()
                .map(|l| LessonProgressDto {
                    lesson: l.lesson.into(),
                    status: status(l.status),
                })
                .collect(),
            active_attempt_ids: p
                .active_attempt_ids
                .into_iter()
                .map(|(t, id)| (assessment_type(t), id))
                .collect(),
            latest_measurement_id: p.latest_measurement_id,
        }
    }
}
#[derive(Serialize)]
pub struct SectionDto {
    id: Uuid,
    title: String,
    body: String,
    required: bool,
    position: i32,
}
#[derive(Serialize)]
pub struct LessonViewDto {
    #[serde(flatten)]
    lesson: LessonDto,
    description: String,
    sections: Vec<SectionDto>,
    visited_section_ids: Vec<Uuid>,
}
impl From<LessonView> for LessonViewDto {
    fn from(v: LessonView) -> Self {
        Self {
            lesson: v.lesson.into(),
            description: v.description,
            sections: v
                .sections
                .into_iter()
                .map(|s| SectionDto {
                    id: s.id,
                    title: s.title,
                    body: s.body,
                    required: s.required,
                    position: s.position,
                })
                .collect(),
            visited_section_ids: v.visited_section_ids,
        }
    }
}
#[derive(Serialize)]
pub struct OpenedDto {
    lesson_id: Uuid,
    opened_at: Option<DateTime<Utc>>,
    visited_section_ids: Vec<Uuid>,
}
impl From<OpenedLesson> for OpenedDto {
    fn from(v: OpenedLesson) -> Self {
        Self {
            lesson_id: v.lesson_id,
            opened_at: v.opened_at,
            visited_section_ids: v.visited_section_ids,
        }
    }
}
#[derive(Serialize)]
pub struct CompletedDto {
    lesson_id: Uuid,
    completed_at: Option<DateTime<Utc>>,
    progress: ProgressDto,
}
impl From<CompletedLesson> for CompletedDto {
    fn from(v: CompletedLesson) -> Self {
        Self {
            lesson_id: v.lesson_id,
            completed_at: v.completed_at,
            progress: v.progress.into(),
        }
    }
}
#[derive(Serialize)]
pub struct AttemptCreationDto {
    attempt_id: Uuid,
    assessment_type: &'static str,
    attempt_number: i32,
    status: String,
    snapshot_id: Uuid,
}
impl From<AttemptCreation> for AttemptCreationDto {
    fn from(v: AttemptCreation) -> Self {
        let a = v.attempt;
        Self {
            attempt_id: a.id,
            assessment_type: assessment_type(a.assessment_type),
            attempt_number: a.attempt_number,
            status: a.status,
            snapshot_id: a.snapshot_id,
        }
    }
}
#[derive(Serialize)]
pub struct AnswerDto {
    snapshot_question_id: Uuid,
    option_ids: Vec<Uuid>,
    revision: i64,
    saved_at: DateTime<Utc>,
}
impl From<Answer> for AnswerDto {
    fn from(a: Answer) -> Self {
        Self {
            snapshot_question_id: a.snapshot_question_id,
            option_ids: a.option_ids,
            revision: a.revision,
            saved_at: a.saved_at,
        }
    }
}
#[derive(Serialize)]
pub struct OptionDto {
    id: Uuid,
    text: String,
}
#[derive(Serialize)]
pub struct QuestionDto {
    id: Uuid,
    question_id: Uuid,
    question_version_id: Uuid,
    position: i32,
    text: String,
    #[serde(rename = "type")]
    question_type: String,
    required: bool,
    options: Vec<OptionDto>,
}
#[derive(Serialize)]
pub struct AttemptViewDto {
    id: Uuid,
    assessment_type: &'static str,
    attempt_number: i32,
    status: String,
    snapshot_id: Uuid,
    submitted_at: Option<DateTime<Utc>>,
    score: Option<i32>,
    questions: Vec<QuestionDto>,
    answers: Vec<AnswerDto>,
}
impl From<AttemptView> for AttemptViewDto {
    fn from(v: AttemptView) -> Self {
        let a = v.attempt;
        Self {
            id: a.id,
            assessment_type: assessment_type(a.assessment_type),
            attempt_number: a.attempt_number,
            status: a.status,
            snapshot_id: a.snapshot_id,
            submitted_at: a.submitted_at,
            score: a.score,
            questions: v
                .questions
                .into_iter()
                .map(|q| QuestionDto {
                    id: q.id,
                    question_id: q.question_id,
                    question_version_id: q.question_version_id,
                    position: q.position,
                    text: q.text,
                    question_type: q.question_type,
                    required: q.required,
                    options: q
                        .options
                        .into_iter()
                        .map(|o| OptionDto {
                            id: o.id,
                            text: o.text,
                        })
                        .collect(),
                })
                .collect(),
            answers: v.answers.into_iter().map(Into::into).collect(),
        }
    }
}
#[derive(Serialize)]
pub struct SubmissionDto {
    attempt_id: Uuid,
    status: &'static str,
    submitted_at: Option<DateTime<Utc>>,
    score: Option<i32>,
    progress: ProgressDto,
}
impl From<Submission> for SubmissionDto {
    fn from(v: Submission) -> Self {
        Self {
            attempt_id: v.attempt_id,
            status: "submitted",
            submitted_at: v.submitted_at,
            score: v.score,
            progress: v.progress.into(),
        }
    }
}
#[derive(Serialize)]
pub struct MeasurementDto {
    id: Uuid,
    weight_kg: Number,
    height_cm: Number,
    bmi: Number,
    bmi_display: String,
    calculation_rule_version: String,
    created_at: DateTime<Utc>,
}
impl TryFrom<Measurement> for MeasurementDto {
    type Error = AppError;
    fn try_from(m: Measurement) -> AppResult<Self> {
        Ok(Self {
            id: m.id,
            weight_kg: m
                .weight_kg
                .decimal_text()
                .parse()
                .map_err(|_| AppError::Internal)?,
            height_cm: m
                .height_cm
                .decimal_text()
                .parse()
                .map_err(|_| AppError::Internal)?,
            bmi: m
                .bmi
                .decimal_text()
                .parse()
                .map_err(|_| AppError::Internal)?,
            bmi_display: m.bmi_display,
            calculation_rule_version: m.calculation_rule_version,
            created_at: m.created_at,
        })
    }
}
#[derive(Serialize)]
pub struct MeasurementCreationDto {
    #[serde(flatten)]
    measurement: MeasurementDto,
    progress: ProgressDto,
}
impl TryFrom<MeasurementCreation> for MeasurementCreationDto {
    type Error = AppError;
    fn try_from(v: MeasurementCreation) -> AppResult<Self> {
        Ok(Self {
            measurement: v.measurement.try_into()?,
            progress: v.progress.into(),
        })
    }
}
#[derive(Serialize)]
pub struct SummaryDto {
    #[serde(flatten)]
    profile: ProfileDto,
    scores: std::collections::BTreeMap<&'static str, Option<i32>>,
    latest_measurement: Option<MeasurementDto>,
    progress: ProgressDto,
}
impl TryFrom<Summary> for SummaryDto {
    type Error = AppError;
    fn try_from(v: Summary) -> AppResult<Self> {
        Ok(Self {
            profile: v.profile.into(),
            scores: v
                .scores
                .into_iter()
                .map(|(t, s)| (assessment_type(t), s))
                .collect(),
            latest_measurement: v.latest_measurement.map(TryInto::try_into).transpose()?,
            progress: v.progress.into(),
        })
    }
}

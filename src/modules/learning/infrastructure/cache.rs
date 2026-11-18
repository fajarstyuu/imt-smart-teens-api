//! Versioned JSONB operation storage, independent of public HTTP DTOs.
//! A version mismatch/corrupt row fails closed; it never becomes permission to repeat writes.
use crate::{
    modules::{
        anthropometry::domain::{Bmi, Measurement, PositiveInput},
        assessment::domain::AssessmentType,
        learning::{application::results::*, domain::*},
    },
    shared::error::{AppError, AppResult},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredStudent {
    id: Uuid,
    name: String,
}
impl From<Student> for StoredStudent {
    fn from(v: Student) -> Self {
        Self {
            id: v.id,
            name: v.name,
        }
    }
}
impl From<StoredStudent> for Student {
    fn from(v: StoredStudent) -> Self {
        Self {
            id: v.id,
            name: v.name,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredEnrollment {
    id: Uuid,
    school_id: Uuid,
    school_name_at_enrollment: String,
    form_date: NaiveDate,
    path_version_id: Uuid,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}
impl From<Enrollment> for StoredEnrollment {
    fn from(v: Enrollment) -> Self {
        Self {
            id: v.id,
            school_id: v.school_id,
            school_name_at_enrollment: v.school_name_at_enrollment,
            form_date: v.form_date,
            path_version_id: v.path_version_id,
            created_at: v.created_at,
            completed_at: v.completed_at,
        }
    }
}
impl From<StoredEnrollment> for Enrollment {
    fn from(v: StoredEnrollment) -> Self {
        Self {
            id: v.id,
            school_id: v.school_id,
            school_name_at_enrollment: v.school_name_at_enrollment,
            form_date: v.form_date,
            path_version_id: v.path_version_id,
            created_at: v.created_at,
            completed_at: v.completed_at,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProfile {
    student: StoredStudent,
    enrollment: StoredEnrollment,
}
impl From<StudentProfile> for StoredProfile {
    fn from(v: StudentProfile) -> Self {
        Self {
            student: v.student.into(),
            enrollment: v.enrollment.into(),
        }
    }
}
impl From<StoredProfile> for StudentProfile {
    fn from(v: StoredProfile) -> Self {
        Self {
            student: v.student.into(),
            enrollment: v.enrollment.into(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLesson {
    lesson_id: Uuid,
    lesson_version_id: Uuid,
    title: String,
    position: i32,
    opened_at: Option<DateTime<Utc>>,
    #[serde(default)]
    reading_completed_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}
impl From<Lesson> for StoredLesson {
    fn from(v: Lesson) -> Self {
        Self {
            lesson_id: v.lesson_id,
            lesson_version_id: v.lesson_version_id,
            title: v.title,
            position: v.position,
            opened_at: v.opened_at,
            reading_completed_at: v.reading_completed_at,
            completed_at: v.completed_at,
        }
    }
}
impl From<StoredLesson> for Lesson {
    fn from(v: StoredLesson) -> Self {
        Self {
            lesson_id: v.lesson_id,
            lesson_version_id: v.lesson_version_id,
            title: v.title,
            position: v.position,
            opened_at: v.opened_at,
            reading_completed_at: v.reading_completed_at,
            completed_at: v.completed_at,
        }
    }
}
#[derive(Serialize, Deserialize)]
enum StoredStage {
    PreTest,
    Lessons,
    Quiz,
    Calculator,
    PostTest,
    Completed,
}
impl From<Stage> for StoredStage {
    fn from(v: Stage) -> Self {
        match v {
            Stage::PreTest => Self::PreTest,
            Stage::Lessons => Self::Lessons,
            Stage::Calculator => Self::Calculator,
            Stage::PostTest => Self::PostTest,
            Stage::Completed => Self::Completed,
        }
    }
}
impl From<StoredStage> for Stage {
    fn from(v: StoredStage) -> Self {
        match v {
            StoredStage::PreTest => Self::PreTest,
            StoredStage::Lessons | StoredStage::Quiz => Self::Lessons,
            StoredStage::Calculator => Self::Calculator,
            StoredStage::PostTest => Self::PostTest,
            StoredStage::Completed => Self::Completed,
        }
    }
}
#[derive(Serialize, Deserialize)]
enum StoredStatus {
    Locked,
    NotStarted,
    InProgress,
    Completed,
}
impl From<ProgressStatus> for StoredStatus {
    fn from(v: ProgressStatus) -> Self {
        match v {
            ProgressStatus::Locked => Self::Locked,
            ProgressStatus::NotStarted => Self::NotStarted,
            ProgressStatus::InProgress => Self::InProgress,
            ProgressStatus::Completed => Self::Completed,
        }
    }
}
impl From<StoredStatus> for ProgressStatus {
    fn from(v: StoredStatus) -> Self {
        match v {
            StoredStatus::Locked => Self::Locked,
            StoredStatus::NotStarted => Self::NotStarted,
            StoredStatus::InProgress => Self::InProgress,
            StoredStatus::Completed => Self::Completed,
        }
    }
}
#[derive(Serialize, Deserialize)]
enum StoredAssessment {
    PreTest,
    Quiz,
    PostTest,
}
impl From<AssessmentType> for StoredAssessment {
    fn from(v: AssessmentType) -> Self {
        match v {
            AssessmentType::PreTest => Self::PreTest,
            AssessmentType::Quiz => Self::Quiz,
            AssessmentType::PostTest => Self::PostTest,
        }
    }
}
impl From<StoredAssessment> for AssessmentType {
    fn from(v: StoredAssessment) -> Self {
        match v {
            StoredAssessment::PreTest => Self::PreTest,
            StoredAssessment::Quiz => Self::Quiz,
            StoredAssessment::PostTest => Self::PostTest,
        }
    }
}
#[derive(Serialize, Deserialize)]
enum StoredCreation {
    Created,
    Existing,
}
impl From<Creation> for StoredCreation {
    fn from(v: Creation) -> Self {
        match v {
            Creation::Created => Self::Created,
            Creation::Existing => Self::Existing,
        }
    }
}
impl From<StoredCreation> for Creation {
    fn from(v: StoredCreation) -> Self {
        match v {
            StoredCreation::Created => Self::Created,
            StoredCreation::Existing => Self::Existing,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredStageProgress {
    stage: StoredStage,
    status: StoredStatus,
    prerequisite_missing: bool,
}
impl From<StageProgress> for StoredStageProgress {
    fn from(v: StageProgress) -> Self {
        Self {
            stage: v.stage.into(),
            status: v.status.into(),
            prerequisite_missing: v.prerequisite_missing,
        }
    }
}
impl From<StoredStageProgress> for StageProgress {
    fn from(v: StoredStageProgress) -> Self {
        Self {
            stage: v.stage.into(),
            status: v.status.into(),
            prerequisite_missing: v.prerequisite_missing,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLessonProgress {
    lesson: StoredLesson,
    status: StoredStatus,
}
impl From<LessonProgress> for StoredLessonProgress {
    fn from(v: LessonProgress) -> Self {
        Self {
            lesson: v.lesson.into(),
            status: v.status.into(),
        }
    }
}
impl From<StoredLessonProgress> for LessonProgress {
    fn from(v: StoredLessonProgress) -> Self {
        Self {
            lesson: v.lesson.into(),
            status: v.status.into(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredProgress {
    enrollment_id: Uuid,
    path_version_id: Uuid,
    current_stage: StoredStage,
    stages: Vec<StoredStageProgress>,
    lessons: Vec<StoredLessonProgress>,
    active_attempt_ids: Vec<(StoredAssessment, Uuid)>,
    latest_measurement_id: Option<Uuid>,
}
impl From<Progress> for StoredProgress {
    fn from(v: Progress) -> Self {
        Self {
            enrollment_id: v.enrollment_id,
            path_version_id: v.path_version_id,
            current_stage: v.current_stage.into(),
            stages: v.stages.into_iter().map(Into::into).collect(),
            lessons: v.lessons.into_iter().map(Into::into).collect(),
            active_attempt_ids: v
                .active_attempt_ids
                .into_iter()
                .map(|(t, id)| (t.into(), id))
                .collect(),
            latest_measurement_id: v.latest_measurement_id,
        }
    }
}
impl From<StoredProgress> for Progress {
    fn from(v: StoredProgress) -> Self {
        Self {
            enrollment_id: v.enrollment_id,
            path_version_id: v.path_version_id,
            current_stage: v.current_stage.into(),
            stages: v.stages.into_iter().map(Into::into).collect(),
            lessons: v.lessons.into_iter().map(Into::into).collect(),
            active_attempt_ids: v
                .active_attempt_ids
                .into_iter()
                .map(|(t, id)| (t.into(), id))
                .collect(),
            latest_measurement_id: v.latest_measurement_id,
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAttempt {
    id: Uuid,
    attempt_number: i32,
    status: String,
    snapshot_id: Uuid,
    assessment_type: StoredAssessment,
}
impl From<AttemptDescriptor> for StoredAttempt {
    fn from(v: AttemptDescriptor) -> Self {
        Self {
            id: v.id,
            attempt_number: v.attempt_number,
            status: v.status,
            snapshot_id: v.snapshot_id,
            assessment_type: v.assessment_type.into(),
        }
    }
}
impl From<StoredAttempt> for AttemptDescriptor {
    fn from(v: StoredAttempt) -> Self {
        Self {
            id: v.id,
            attempt_number: v.attempt_number,
            status: v.status,
            snapshot_id: v.snapshot_id,
            assessment_type: v.assessment_type.into(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAttemptCreation {
    outcome: StoredCreation,
    attempt: StoredAttempt,
}
impl From<AttemptCreation> for StoredAttemptCreation {
    fn from(v: AttemptCreation) -> Self {
        Self {
            outcome: v.outcome.into(),
            attempt: v.attempt.into(),
        }
    }
}
impl From<StoredAttemptCreation> for AttemptCreation {
    fn from(v: StoredAttemptCreation) -> Self {
        Self {
            outcome: v.outcome.into(),
            attempt: v.attempt.into(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredSubmission {
    attempt_id: Uuid,
    submitted_at: Option<DateTime<Utc>>,
    score: Option<i32>,
    progress: StoredProgress,
}
impl From<Submission> for StoredSubmission {
    fn from(v: Submission) -> Self {
        Self {
            attempt_id: v.attempt_id,
            submitted_at: v.submitted_at,
            score: v.score,
            progress: v.progress.into(),
        }
    }
}
impl From<StoredSubmission> for Submission {
    fn from(v: StoredSubmission) -> Self {
        Self {
            attempt_id: v.attempt_id,
            submitted_at: v.submitted_at,
            score: v.score,
            progress: v.progress.into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredMeasurement {
    id: Uuid,
    weight: String,
    height: String,
    bmi: String,
    display: String,
    rule: String,
    created_at: DateTime<Utc>,
}
impl From<Measurement> for StoredMeasurement {
    fn from(m: Measurement) -> Self {
        Self {
            id: m.id,
            weight: m.weight_kg.decimal_text(),
            height: m.height_cm.decimal_text(),
            bmi: m.bmi.decimal_text(),
            display: m.bmi_display,
            rule: m.calculation_rule_version,
            created_at: m.created_at,
        }
    }
}
impl TryFrom<StoredMeasurement> for Measurement {
    type Error = AppError;
    fn try_from(m: StoredMeasurement) -> AppResult<Self> {
        Ok(Self {
            id: m.id,
            weight_kg: PositiveInput::parse_text(&m.weight).map_err(|_| AppError::Internal)?,
            height_cm: PositiveInput::parse_text(&m.height).map_err(|_| AppError::Internal)?,
            bmi: Bmi::parse_text(&m.bmi).map_err(|_| AppError::Internal)?,
            bmi_display: m.display,
            calculation_rule_version: m.rule,
            created_at: m.created_at,
        })
    }
}
#[derive(Serialize, Deserialize)]
enum StoredResult {
    Enrollment(StoredProfile),
    Attempt(StoredAttemptCreation),
    Submission(StoredSubmission),
    Measurement {
        measurement: StoredMeasurement,
        progress: StoredProgress,
    },
    LessonQuizAnswer(StoredLessonQuizAnswer),
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredLessonQuizAnswer {
    lesson_id: Uuid,
    lesson_version_id: Uuid,
    selected_option_id: Uuid,
    correct: bool,
    first_answer_option_id: Uuid,
    attempt_count: i32,
    revision: i64,
    mastered: bool,
    progress: StoredProgress,
}
impl From<LessonQuizAnswerResult> for StoredLessonQuizAnswer {
    fn from(v: LessonQuizAnswerResult) -> Self {
        Self {
            lesson_id: v.lesson_id,
            lesson_version_id: v.lesson_version_id,
            selected_option_id: v.selected_option_id,
            correct: v.correct,
            first_answer_option_id: v.state.first_answer_option_id,
            attempt_count: v.state.attempt_count,
            revision: v.state.revision,
            mastered: v.state.mastered,
            progress: v.progress.into(),
        }
    }
}
impl From<StoredLessonQuizAnswer> for LessonQuizAnswerResult {
    fn from(v: StoredLessonQuizAnswer) -> Self {
        Self {
            lesson_id: v.lesson_id,
            lesson_version_id: v.lesson_version_id,
            selected_option_id: v.selected_option_id,
            correct: v.correct,
            state: LessonQuizAnswerState {
                first_answer_option_id: v.first_answer_option_id,
                attempt_count: v.attempt_count,
                revision: v.revision,
                mastered: v.mastered,
            },
            progress: v.progress.into(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheV1 {
    version: u32,
    result: StoredResult,
}
/// Encode semantic outcomes, keeping transport status only in the legacy relational column.
pub(super) fn encode(result: OperationResult) -> AppResult<(i32, serde_json::Value)> {
    let (status, result) = match result {
        OperationResult::Enrollment(v) => (201, StoredResult::Enrollment(v.into())),
        OperationResult::Attempt(v) => (
            if v.outcome == Creation::Created {
                201
            } else {
                200
            },
            StoredResult::Attempt(v.into()),
        ),
        OperationResult::Submission(v) => (200, StoredResult::Submission(v.into())),
        OperationResult::Measurement(v) => (
            201,
            StoredResult::Measurement {
                measurement: v.measurement.into(),
                progress: v.progress.into(),
            },
        ),
        OperationResult::LessonQuizAnswer(v) => (200, StoredResult::LessonQuizAnswer(v.into())),
    };
    Ok((
        status,
        serde_json::to_value(CacheV1 { version: 1, result }).map_err(|_| AppError::Internal)?,
    ))
}
pub(super) fn decode(status: i32, body: serde_json::Value) -> AppResult<OperationResult> {
    let row: CacheV1 = serde_json::from_value(body).map_err(|_| AppError::Internal)?;
    if row.version != 1 {
        return Err(AppError::Internal);
    }
    let (expected, result) = match row.result {
        StoredResult::Enrollment(v) => (201, OperationResult::Enrollment(v.into())),
        StoredResult::Attempt(v) => {
            let v: AttemptCreation = v.into();
            (
                if v.outcome == Creation::Created {
                    201
                } else {
                    200
                },
                OperationResult::Attempt(v),
            )
        }
        StoredResult::Submission(v) => (200, OperationResult::Submission(v.into())),
        StoredResult::Measurement {
            measurement,
            progress,
        } => (
            201,
            OperationResult::Measurement(MeasurementCreation {
                measurement: measurement.try_into()?,
                progress: progress.into(),
            }),
        ),
        StoredResult::LessonQuizAnswer(v) => (200, OperationResult::LessonQuizAnswer(v.into())),
    };
    if status != expected {
        return Err(AppError::Internal);
    }
    Ok(result)
}

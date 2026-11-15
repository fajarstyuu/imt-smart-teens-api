//! Explicit report schemas and exact decimal conversion at HTTP only. Reuses existing
//! safe student/enrollment/lesson/measurement allowlists, never admin editor keys.
use super::super::application::*;
use crate::{
    modules::learning::presentation::dto::{EnrollmentDto, LessonDto, MeasurementDto, StudentDto},
    shared::error::{AppError, AppResult},
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use serde_json::Number;
use uuid::Uuid;
#[derive(Serialize)]
pub struct StudentSummaryDto {
    student_id: Uuid,
    name: String,
    enrollment_id: Uuid,
    school_id: Uuid,
    school_name_at_enrollment: String,
    form_date: NaiveDate,
    path_version_id: Uuid,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    current_stage: &'static str,
    latest_submitted_at: Option<DateTime<Utc>>,
}
#[derive(Serialize)]
pub struct AttemptSummaryDto {
    id: Uuid,
    snapshot_id: Uuid,
    assessment_type: &'static str,
    attempt_number: i32,
    status: String,
    created_at: DateTime<Utc>,
    submitted_at: Option<DateTime<Utc>>,
    earned_points: Option<Number>,
    max_points: Option<Number>,
    score_percent: Option<i32>,
}
#[derive(Serialize)]
pub struct StudentResultsDto {
    student: StudentDto,
    enrollment: EnrollmentDto,
    current_stage: &'static str,
    latest_submitted_at: Option<DateTime<Utc>>,
    attempts: Vec<AttemptSummaryDto>,
    lessons: Vec<LessonDto>,
    measurements: Vec<MeasurementDto>,
}
#[derive(Serialize)]
pub struct ReportOptionDto {
    id: Uuid,
    text: String,
}
#[derive(Serialize)]
pub struct QuestionAnswerDto {
    student_id: Uuid,
    student_name: String,
    enrollment_id: Uuid,
    school_id: Uuid,
    school_name_at_enrollment: String,
    bank_id: Uuid,
    bank_version_id: Uuid,
    snapshot_id: Uuid,
    snapshot_question_id: Uuid,
    question_id: Uuid,
    question_version_id: Uuid,
    position: i32,
    text: String,
    #[serde(rename = "type")]
    question_type: String,
    assessment_type: &'static str,
    attempt_id: Option<Uuid>,
    attempt_number: Option<i32>,
    attempt_status: Option<String>,
    answer_revision: Option<i64>,
    saved_at: Option<DateTime<Utc>>,
    submitted_at: Option<DateTime<Utc>>,
    options: Vec<ReportOptionDto>,
    selected_option_ids: Vec<Uuid>,
    selected_options: Vec<ReportOptionDto>,
    answer_state: &'static str,
    correctness: Option<bool>,
    earned_points: Option<Number>,
    max_points: Number,
}
/// Number retains scale4 numeric tokens (not strings/floats). Option fields deliberately
/// serialize null rather than disappear, including pending grades and missing attempts.
fn points(p: Points) -> AppResult<Number> {
    p.decimal().parse().map_err(|_| AppError::Internal)
}
pub fn summary_dto(s: StudentSummary) -> StudentSummaryDto {
    StudentSummaryDto {
        student_id: s.student.id,
        name: s.student.name,
        enrollment_id: s.enrollment.id,
        school_id: s.enrollment.school_id,
        school_name_at_enrollment: s.enrollment.school_name_at_enrollment,
        form_date: s.enrollment.form_date,
        path_version_id: s.enrollment.path_version_id,
        created_at: s.enrollment.created_at,
        completed_at: s.enrollment.completed_at,
        current_stage: stage_name(s.stage),
        latest_submitted_at: s.latest_submitted_at,
    }
}
fn attempt_dto(a: AttemptSummary) -> AppResult<AttemptSummaryDto> {
    Ok(AttemptSummaryDto {
        id: a.id,
        snapshot_id: a.snapshot_id,
        assessment_type: a.assessment_type.as_str(),
        attempt_number: a.attempt_number,
        status: a.status,
        created_at: a.created_at,
        submitted_at: a.submitted_at,
        earned_points: a.earned_points.map(points).transpose()?,
        max_points: a.max_points.map(points).transpose()?,
        score_percent: a.score_percent,
    })
}
pub fn results_dto(r: StudentResults) -> AppResult<StudentResultsDto> {
    Ok(StudentResultsDto {
        student: r.summary.student.into(),
        enrollment: r.summary.enrollment.into(),
        current_stage: stage_name(r.summary.stage),
        latest_submitted_at: r.summary.latest_submitted_at,
        attempts: r
            .attempts
            .into_iter()
            .map(attempt_dto)
            .collect::<AppResult<_>>()?,
        lessons: r.lessons.into_iter().map(Into::into).collect(),
        measurements: r
            .measurements
            .into_iter()
            .map(TryInto::try_into)
            .collect::<AppResult<_>>()?,
    })
}
fn option_dto(o: ReportOption) -> ReportOptionDto {
    ReportOptionDto {
        id: o.id,
        text: o.text,
    }
}
pub fn answer_dto(a: QuestionAnswer) -> AppResult<QuestionAnswerDto> {
    let state = a.state().as_str();
    Ok(QuestionAnswerDto {
        student_id: a.student_id,
        student_name: a.student_name,
        enrollment_id: a.enrollment_id,
        school_id: a.school_id,
        school_name_at_enrollment: a.school_name_at_enrollment,
        bank_id: a.bank_id,
        bank_version_id: a.bank_version_id,
        snapshot_id: a.snapshot_id,
        snapshot_question_id: a.snapshot_question_id,
        question_id: a.question_id,
        question_version_id: a.question_version_id,
        position: a.position,
        text: a.text,
        question_type: a.question_type,
        assessment_type: a.assessment_type.as_str(),
        attempt_id: a.attempt_id,
        attempt_number: a.attempt_number,
        attempt_status: a.attempt_status,
        answer_revision: a.answer_revision,
        saved_at: a.saved_at,
        submitted_at: a.submitted_at,
        options: a.options.into_iter().map(option_dto).collect(),
        selected_option_ids: a.selected_options.iter().map(|o| o.id).collect(),
        selected_options: a.selected_options.into_iter().map(option_dto).collect(),
        answer_state: state,
        correctness: a.correctness,
        earned_points: a.earned_points.map(points).transpose()?,
        max_points: points(a.max_points)?,
    })
}

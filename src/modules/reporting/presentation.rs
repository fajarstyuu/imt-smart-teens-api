//! Authenticated report handlers, strict allowlisted query parsing and exact numeric DTOs.
use super::application::*;
use crate::{
    modules::learning::presentation::dto::{EnrollmentDto, LessonDto, MeasurementDto, StudentDto},
    shared::error::{AppError, AppResult},
    transport::http::{
        AppState,
        error::ApiError,
        extractors::{AdminSession, ApiPath},
    },
};
use axum::{
    Json, Router,
    extract::{FromRequestParts, Query, State},
    http::{StatusCode, request::Parts},
    routing::get,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::{Number, Value, json};
use std::{collections::BTreeMap, str::FromStr};
use uuid::Uuid;
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/students", get(students))
        .route("/api/v1/admin/students/{student}/results", get(results))
        .route("/api/v1/admin/attempts/{attempt}/answers", get(attempt))
        .route("/api/v1/admin/reports/question-answers", get(question))
}
/// Values are parsed only after endpoint-specific field allowlisting. Unlike generic
/// transport queries, unknown/duplicate or invalid report filters intentionally yield422.
struct ReportQuery(BTreeMap<String, String>);
impl FromRequestParts<AppState> for ReportQuery {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Query(pairs) = Query::<Vec<(String, String)>>::from_request_parts(parts, state)
            .await
            .map_err(|_| {
                ApiError::transport(StatusCode::UNPROCESSABLE_ENTITY, "INVALID_REPORT_FILTER")
            })?;
        let mut values = BTreeMap::new();
        for (k, v) in pairs {
            if values.insert(k, v).is_some() {
                return Err(AppError::Invalid("INVALID_REPORT_FILTER").into());
            }
        }
        Ok(Self(values))
    }
}
impl ReportQuery {
    fn only(&self, keys: &[&str]) -> AppResult<()> {
        if self.0.keys().any(|k| !keys.contains(&k.as_str())) {
            Err(AppError::Invalid("INVALID_REPORT_FILTER"))
        } else {
            Ok(())
        }
    }
    fn parse<T: FromStr>(&self, key: &str) -> AppResult<Option<T>> {
        self.0
            .get(key)
            .map(|s| {
                s.parse()
                    .map_err(|_| AppError::Invalid("INVALID_REPORT_FILTER"))
            })
            .transpose()
    }
    fn date(&self, key: &str) -> AppResult<Option<NaiveDate>> {
        let Some(s) = self.0.get(key) else {
            return Ok(None);
        };
        let d = NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| AppError::Invalid("INVALID_REPORT_DATE"))?;
        if d.format("%Y-%m-%d").to_string() != *s {
            return Err(AppError::Invalid("INVALID_REPORT_DATE"));
        }
        Ok(Some(d))
    }
    fn time(&self, key: &str) -> AppResult<Option<DateTime<Utc>>> {
        let Some(s) = self.0.get(key) else {
            return Ok(None);
        };
        let t = DateTime::parse_from_rfc3339(s)
            .map_err(|_| AppError::Invalid("INVALID_REPORT_TIMESTAMP"))?;
        if t.offset().local_minus_utc() != 0 || !(s.ends_with('Z') || s.ends_with("+00:00")) {
            return Err(AppError::Invalid("INVALID_REPORT_TIMESTAMP"));
        }
        Ok(Some(t.with_timezone(&Utc)))
    }
    fn filters(&self) -> AppResult<Filters> {
        Ok(Filters {
            school_id: self.parse("school_id")?,
            form_date_from: self.date("form_date_from")?,
            form_date_to: self.date("form_date_to")?,
            submitted_from: self.time("submitted_from")?,
            submitted_to: self.time("submitted_to")?,
            stage: self.0.get("stage").map(|s| parse_stage(s)).transpose()?,
            assessment_type: self
                .0
                .get("assessment_type")
                .map(|s| parse_type(s))
                .transpose()?,
        })
    }
    fn window(&self) -> AppResult<Window> {
        Ok(Window {
            limit: self.parse("limit")?.unwrap_or(25),
            offset: self.parse("offset")?.unwrap_or(0),
        })
    }
}
fn points(p: Points) -> AppResult<Number> {
    p.decimal().parse().map_err(|_| AppError::Internal)
}
fn summary_dto(s: StudentSummary) -> Value {
    json!({"student_id":s.student.id,"name":s.student.name,"enrollment_id":s.enrollment.id,"school_id":s.enrollment.school_id,"school_name_at_enrollment":s.enrollment.school_name_at_enrollment,"form_date":s.enrollment.form_date,"path_version_id":s.enrollment.path_version_id,"created_at":s.enrollment.created_at,"completed_at":s.enrollment.completed_at,"current_stage":stage_name(s.stage),"latest_submitted_at":s.latest_submitted_at})
}
fn attempt_dto(a: AttemptSummary) -> AppResult<Value> {
    Ok(
        json!({"id":a.id,"snapshot_id":a.snapshot_id,"assessment_type":a.assessment_type.as_str(),"attempt_number":a.attempt_number,"status":a.status,"created_at":a.created_at,"submitted_at":a.submitted_at,"earned_points":a.earned_points.map(points).transpose()?,"max_points":a.max_points.map(points).transpose()?,"score_percent":a.score_percent}),
    )
}
fn answer_dto(a: QuestionAnswer) -> AppResult<Value> {
    let state = a.state().as_str();
    Ok(
        json!({"student_id":a.student_id,"student_name":a.student_name,"enrollment_id":a.enrollment_id,"school_id":a.school_id,"school_name_at_enrollment":a.school_name_at_enrollment,"bank_id":a.bank_id,"bank_version_id":a.bank_version_id,"snapshot_id":a.snapshot_id,"snapshot_question_id":a.snapshot_question_id,"question_id":a.question_id,"question_version_id":a.question_version_id,"position":a.position,"text":a.text,"type":a.question_type,"assessment_type":a.assessment_type.as_str(),"attempt_id":a.attempt_id,"attempt_number":a.attempt_number,"attempt_status":a.attempt_status,"answer_revision":a.answer_revision,"saved_at":a.saved_at,"submitted_at":a.submitted_at,"options":a.options.into_iter().map(|o|json!({"id":o.id,"text":o.text})).collect::<Vec<_>>(),"selected_option_ids":a.selected_options.iter().map(|o|o.id).collect::<Vec<_>>(),"selected_options":a.selected_options.into_iter().map(|o|json!({"id":o.id,"text":o.text})).collect::<Vec<_>>(),"answer_state":state,"correctness":a.correctness,"earned_points":a.earned_points.map(points).transpose()?,"max_points":points(a.max_points)?}),
    )
}
async fn students(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    q: ReportQuery,
) -> Result<Json<Value>, ApiError> {
    q.only(&[
        "school_id",
        "form_date_from",
        "form_date_to",
        "submitted_from",
        "submitted_to",
        "stage",
        "assessment_type",
        "limit",
        "offset",
    ])?;
    let w = q.window()?;
    let p = s.reports.students(&a.principal, q.filters()?, w).await?;
    Ok(Json(
        json!({"data":p.rows.into_iter().map(summary_dto).collect::<Vec<_>>(),"meta":{"limit":w.limit,"offset":w.offset,"total":p.total}}),
    ))
}
async fn results(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    q: ReportQuery,
) -> Result<Json<Value>, ApiError> {
    q.only(&["enrollment_id"])?;
    let r = s
        .reports
        .results(&a.principal, id, q.parse("enrollment_id")?)
        .await?;
    Ok(Json(
        json!({"data":{"student":StudentDto::from(r.summary.student),"enrollment":EnrollmentDto::from(r.summary.enrollment),"current_stage":stage_name(r.summary.stage),"latest_submitted_at":r.summary.latest_submitted_at,"attempts":r.attempts.into_iter().map(attempt_dto).collect::<AppResult<Vec<_>>>()?,"lessons":r.lessons.into_iter().map(LessonDto::from).collect::<Vec<_>>(),"measurements":r.measurements.into_iter().map(MeasurementDto::try_from).collect::<AppResult<Vec<_>>>()?}}),
    ))
}
async fn attempt(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    q: ReportQuery,
) -> Result<Json<Value>, ApiError> {
    q.only(&["limit", "offset"])?;
    let w = q.window()?;
    let p = s.reports.attempt_answers(&a.principal, id, w).await?;
    Ok(Json(
        json!({"data":p.rows.into_iter().map(answer_dto).collect::<AppResult<Vec<_>>>()?,"meta":{"limit":w.limit,"offset":w.offset,"total":p.total}}),
    ))
}
async fn question(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    q: ReportQuery,
) -> Result<Json<Value>, ApiError> {
    q.only(&[
        "bank_id",
        "question_id",
        "question_version_id",
        "assessment_type",
        "school_id",
        "submitted_from",
        "submitted_to",
        "limit",
        "offset",
    ])?;
    let w = q.window()?;
    let p = s
        .reports
        .question_answers(
            &a.principal,
            q.parse("question_version_id")?,
            q.parse("question_id")?,
            q.parse("bank_id")?,
            q.filters()?,
            w,
        )
        .await?;
    Ok(Json(
        json!({"data":p.rows.into_iter().map(answer_dto).collect::<AppResult<Vec<_>>>()?,"meta":{"limit":w.limit,"offset":w.offset,"total":p.total}}),
    ))
}

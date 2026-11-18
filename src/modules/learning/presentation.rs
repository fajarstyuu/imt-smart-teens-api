//! Thin v1 student routes. Extractors authenticate; application facades own all policy.
pub(crate) mod dto;
use super::super::identity::presentation::{student_logout, student_register};
use crate::{
    modules::{
        assessment::domain::AssessmentType,
        identity::application::Authenticated,
        learning::{
            application::{Creation, Operation},
            domain::StudentScope,
        },
    },
    transport::http::{
        AppState,
        csrf::Csrf,
        error::{ApiError, REQUEST_ID},
        extractors::{ApiJson, ApiPath, ApiQuery, StudentSession},
        pagination::Pagination,
    },
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use dto::*;
use serde::Deserialize;
use serde_json::{Number, Value, json};
use uuid::Uuid;
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/student/sessions", post(student_register))
        .route("/api/v1/student/sessions/current", delete(student_logout))
        .route("/api/v1/student/me", get(me))
        .route("/api/v1/student/progress", get(progress))
        .route("/api/v1/student/lessons/{lesson_id}", get(lesson))
        .route("/api/v1/student/lessons/{lesson_id}/quiz", get(lesson_quiz))
        .route(
            "/api/v1/student/lessons/{lesson_id}/quiz/answers",
            post(answer_lesson_quiz),
        )
        .route("/api/v1/student/lessons/{lesson_id}/opened", post(opened))
        .route(
            "/api/v1/student/lessons/{lesson_id}/completion",
            post(completion),
        )
        .route("/api/v1/student/attempts", post(create_attempt))
        .route("/api/v1/student/attempts/{attempt_id}", get(attempt))
        .route(
            "/api/v1/student/attempts/{attempt_id}/answers/{snapshot_question_id}",
            put(save_answer),
        )
        .route(
            "/api/v1/student/attempts/{attempt_id}/submission",
            post(submit),
        )
        .route(
            "/api/v1/student/measurements",
            get(measurements).post(measure),
        )
        .route("/api/v1/student/summary", get(summary))
}
fn scope(auth: &Authenticated) -> Result<StudentScope, ApiError> {
    let (student_id, enrollment_id) = auth.principal.student()?;
    Ok(StudentScope {
        student_id,
        enrollment_id,
    })
}
fn operation(
    auth: &Authenticated,
    headers: &HeaderMap,
    payload: &impl serde::Serialize,
) -> Result<Operation, ApiError> {
    let scope = scope(auth)?;
    Ok(Operation::new(
        Csrf::binding(&scope.enrollment_id.to_string()),
        headers
            .get("idempotency-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .into(),
        &serde_json::to_vec(payload).map_err(|_| crate::shared::error::AppError::Internal)?,
        REQUEST_ID.with(Clone::clone),
    )?)
}
async fn me(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":ProfileDto::from(s.students.me(scope(&a)?).await?)}),
    ))
}
async fn progress(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":ProgressDto::from(s.students.progress(scope(&a)?).await?)}),
    ))
}
async fn lesson(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":LessonViewDto::from(s.students.lesson(scope(&a)?,id).await?)}),
    ))
}
async fn lesson_quiz(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":LessonQuizViewDto::from(s.students.lesson_quiz(scope(&a)?, id).await?)}),
    ))
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct LessonQuizAnswerRequest {
    option_id: Uuid,
    expected_revision: i64,
}
async fn answer_lesson_quiz(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<LessonQuizAnswerRequest>,
) -> Result<Json<Value>, ApiError> {
    let semantic_payload = serde_json::json!({
        "lesson_id": id,
        "option_id": input.option_id,
        "expected_revision": input.expected_revision,
    });
    let result = s
        .students
        .answer_lesson_quiz(
            scope(&a)?,
            id,
            input.option_id,
            input.expected_revision,
            operation(&a, &headers, &semantic_payload)?,
        )
        .await?;
    Ok(Json(json!({"data":LessonQuizAnswerDto::from(result)})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Opened {
    section_id: Option<Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
async fn opened(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<Opened>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":OpenedDto::from(s.students.open_lesson(scope(&a)?,id,input.section_id).await?)}),
    ))
}
async fn completion(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(_): ApiJson<Empty>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":CompletedDto::from(s.students.complete_lesson(scope(&a)?,id).await?)}),
    ))
}
#[derive(Deserialize, serde::Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum AssessmentTypeRequest {
    PreTest,
    Quiz,
    PostTest,
}
impl From<AssessmentTypeRequest> for AssessmentType {
    fn from(t: AssessmentTypeRequest) -> Self {
        match t {
            AssessmentTypeRequest::PreTest => Self::PreTest,
            AssessmentTypeRequest::Quiz => Self::Quiz,
            AssessmentTypeRequest::PostTest => Self::PostTest,
        }
    }
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CreateAttempt {
    assessment_type: AssessmentTypeRequest,
}
async fn create_attempt(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    headers: HeaderMap,
    ApiJson(input): ApiJson<CreateAttempt>,
) -> Result<Response, ApiError> {
    let out = s
        .students
        .create_attempt(
            scope(&a)?,
            input.assessment_type.into(),
            operation(&a, &headers, &input)?,
        )
        .await?;
    Ok((
        if out.outcome == Creation::Created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(json!({"data":AttemptCreationDto::from(out)})),
    )
        .into_response())
}
async fn attempt(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":AttemptViewDto::from(s.students.attempt(scope(&a)?,id).await?)}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveAnswer {
    option_ids: Vec<Uuid>,
    expected_revision: i64,
}
async fn save_answer(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath((id, q)): ApiPath<(Uuid, Uuid)>,
    ApiJson(input): ApiJson<SaveAnswer>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":AnswerDto::from(s.students.save_answer(scope(&a)?,id,q,input.option_ids,input.expected_revision).await?)}),
    ))
}
async fn submit(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
    headers: HeaderMap,
    ApiJson(_): ApiJson<Empty>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":SubmissionDto::from(s.students.submit(scope(&a)?,id,operation(&a,&headers,&json!({}))?).await?)}),
    ))
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Measure {
    weight_kg: Number,
    height_cm: Number,
}
async fn measure(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    headers: HeaderMap,
    ApiJson(input): ApiJson<Measure>,
) -> Result<Response, ApiError> {
    let op = operation(&a, &headers, &input)?;
    let out = s
        .students
        .measure(
            scope(&a)?,
            crate::modules::anthropometry::domain::PositiveInput::parse_text(
                &input.weight_kg.to_string(),
            )?,
            crate::modules::anthropometry::domain::PositiveInput::parse_text(
                &input.height_cm.to_string(),
            )?,
            op,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"data":MeasurementCreationDto::try_from(out)?})),
    )
        .into_response())
}
async fn measurements(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiQuery(p): ApiQuery<Pagination>,
) -> Result<Json<Value>, ApiError> {
    let (limit, offset) = p.validate()?;
    let history = s.students.measurements(scope(&a)?, limit, offset).await?;
    let rows = history
        .measurements
        .into_iter()
        .map(MeasurementDto::try_from)
        .collect::<crate::shared::error::AppResult<Vec<_>>>()?;
    let total = history.total;
    Ok(Json(
        json!({"data":rows,"meta":{"limit":limit,"offset":offset,"total":total}}),
    ))
}
async fn summary(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":SummaryDto::try_from(s.students.summary(scope(&a)?).await?)?}),
    ))
}

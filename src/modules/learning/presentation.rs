//! Thin v1 student routes. Extractors authenticate; application facades own all policy.
use super::super::identity::presentation::{student_logout, student_register};
use crate::{
    modules::{
        assessment::domain::AssessmentType,
        identity::application::Authenticated,
        learning::{application::Operation, domain::StudentScope},
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
        payload,
        REQUEST_ID.with(Clone::clone),
    )?)
}
async fn me(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data":s.students.me(scope(&a)?).await?})))
}
async fn progress(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data":s.students.progress(scope(&a)?).await?})))
}
async fn lesson(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":s.students.lesson(scope(&a)?,id).await?}),
    ))
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
        json!({"data":s.students.open_lesson(scope(&a)?,id,input.section_id).await?}),
    ))
}
async fn completion(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(_): ApiJson<Empty>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":s.students.complete_lesson(scope(&a)?,id).await?}),
    ))
}
#[derive(Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct CreateAttempt {
    assessment_type: AssessmentType,
}
async fn create_attempt(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    headers: HeaderMap,
    ApiJson(input): ApiJson<CreateAttempt>,
) -> Result<Response, ApiError> {
    let (status, out) = s
        .students
        .create_attempt(
            scope(&a)?,
            input.assessment_type,
            operation(&a, &headers, &input)?,
        )
        .await?;
    Ok((
        StatusCode::from_u16(status).map_err(|_| crate::shared::error::AppError::Internal)?,
        Json(json!({"data":out})),
    )
        .into_response())
}
async fn attempt(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":s.students.attempt(scope(&a)?,id).await?}),
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
        json!({"data":s.students.save_answer(scope(&a)?,id,q,input.option_ids,input.expected_revision).await?}),
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
        json!({"data":s.students.submit(scope(&a)?,id,operation(&a,&headers,&json!({}))?).await?}),
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
        .measure(scope(&a)?, input.weight_kg, input.height_cm, op)
        .await?;
    Ok((StatusCode::CREATED, Json(json!({"data":out}))).into_response())
}
async fn measurements(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
    ApiQuery(p): ApiQuery<Pagination>,
) -> Result<Json<Value>, ApiError> {
    let (limit, offset) = p.validate()?;
    let (rows, total) = s.students.measurements(scope(&a)?, limit, offset).await?;
    Ok(Json(
        json!({"data":rows,"meta":{"limit":limit,"offset":offset,"total":total}}),
    ))
}
async fn summary(
    State(s): State<AppState>,
    StudentSession(a): StudentSession,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({"data":s.students.summary(scope(&a)?).await?})))
}

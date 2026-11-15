//! Thin admin question DTO adapters; private keys are restricted to this authenticated editor.
use super::application::*;
use crate::{
    modules::{
        identity::application::Authenticated,
        learning::admin::{
            application::actor,
            presentation::{ListQuery, Revision, version_dto},
        },
    },
    shared::error::AppError,
    transport::http::{
        AppState,
        error::{ApiError, REQUEST_ID},
        extractors::{AdminSession, ApiJson, ApiPath, ApiQuery},
    },
};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/question-banks", get(banks))
        .route(
            "/api/v1/admin/question-banks/{bank}/questions",
            get(questions).post(create),
        )
        .route(
            "/api/v1/admin/question-banks/{bank}/questions/{id}",
            get(detail).put(update),
        )
        .route(
            "/api/v1/admin/question-banks/{bank}/questions/{id}/archive",
            post(archive),
        )
        .route(
            "/api/v1/admin/question-banks/{bank}/publication",
            post(publish),
        )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionInput {
    id: Option<Uuid>,
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuestionInput {
    text: String,
    #[serde(rename = "type")]
    question_type: String,
    options: Vec<OptionInput>,
    correct_option_ids: Vec<Uuid>,
    position: i32,
    required: bool,
    expected_revision: Option<i64>,
}
impl QuestionInput {
    fn draft(self) -> Result<QuestionDraft, ApiError> {
        Ok(QuestionDraft {
            text: self.text,
            question_type: QuestionType::parse(&self.question_type)?,
            options: self
                .options
                .into_iter()
                .map(|o| OptionDraft {
                    id: o.id.unwrap_or_else(Uuid::new_v4),
                    text: o.text,
                })
                .collect(),
            correct_option_ids: self.correct_option_ids,
            required: self.required,
        })
    }
}
fn audit(a: &Authenticated) -> Result<crate::modules::identity::application::AuditActor, ApiError> {
    Ok(actor(
        &a.principal,
        a.session_id,
        REQUEST_ID.with(Clone::clone),
    )?)
}
pub fn bank_dto(b: BankEditor) -> Value {
    json!({"id":b.id,"kind":b.kind,"state":b.state.as_str(),"revision":b.revision,"versions":b.versions.into_iter().map(version_dto).collect::<Vec<_>>()})
}
fn question_dto(q: QuestionEditor) -> Value {
    json!({"id":q.id,"bank_id":q.bank_id,"state":q.state.as_str(),"revision":q.revision,"position":q.position,"draft":{"text":q.draft.text,"type":q.draft.question_type.as_str(),"options":q.draft.options.into_iter().map(|o|json!({"id":o.id,"text":o.text})).collect::<Vec<_>>(),"correct_option_ids":q.draft.correct_option_ids,"required":q.draft.required},"versions":q.versions.into_iter().map(version_dto).collect::<Vec<_>>()})
}
async fn banks(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
) -> Result<Json<Value>, ApiError> {
    let rows = s.bank_cms.banks(&a.principal).await?;
    let total = rows.len();
    Ok(Json(
        json!({"data":rows.into_iter().map(bank_dto).collect::<Vec<_>>(),"meta":{"limit":2,"offset":0,"total":total}}),
    ))
}
async fn questions(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(bank): ApiPath<Uuid>,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let q = q.core()?;
    let (limit, offset) = (q.limit, q.offset);
    let p = s.bank_cms.questions(&a.principal, bank, q).await?;
    Ok(Json(
        json!({"data":p.rows.into_iter().map(question_dto).collect::<Vec<_>>(),"meta":{"limit":limit,"offset":offset,"total":p.total}}),
    ))
}
async fn detail(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath((b, id)): ApiPath<(Uuid, Uuid)>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":question_dto(s.bank_cms.question(&a.principal,b,id).await?)}),
    ))
}
async fn create(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(b): ApiPath<Uuid>,
    ApiJson(i): ApiJson<QuestionInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if i.expected_revision.is_some() {
        return Err(AppError::Invalid("INVALID_INPUT").into());
    }
    let pos = i.position;
    Ok((
        StatusCode::CREATED,
        Json(
            json!({"data":question_dto(s.bank_cms.create(&a.principal,audit(&a)?,b,i.draft()?,pos).await?)}),
        ),
    ))
}
async fn update(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(ids): ApiPath<(Uuid, Uuid)>,
    ApiJson(i): ApiJson<QuestionInput>,
) -> Result<Json<Value>, ApiError> {
    let rev = i
        .expected_revision
        .ok_or(AppError::Invalid("REVISION_REQUIRED"))?;
    let pos = i.position;
    Ok(Json(
        json!({"data":question_dto(s.bank_cms.update(&a.principal,audit(&a)?,ids,i.draft()?,pos,rev).await?)}),
    ))
}
async fn archive(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(ids): ApiPath<(Uuid, Uuid)>,
    ApiJson(i): ApiJson<Revision>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":question_dto(s.bank_cms.archive(&a.principal,audit(&a)?,ids,i.expected_revision).await?)}),
    ))
}
async fn publish(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(b): ApiPath<Uuid>,
    ApiJson(i): ApiJson<Revision>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":bank_dto(s.bank_cms.publish(&a.principal,audit(&a)?,b,i.expected_revision).await?)}),
    ))
}

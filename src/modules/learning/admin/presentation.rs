//! Admin-only allowlisted editor DTOs. No persistence or SQL is reachable from handlers.
use super::application::*;
use crate::{
    modules::identity::application::Authenticated,
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
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/lessons", get(list).post(create))
        .route("/api/v1/admin/lessons/{id}", get(detail).put(update))
        .route("/api/v1/admin/lessons/{id}/publication", post(publish))
        .route("/api/v1/admin/lessons/{id}/archive", post(archive))
        .route("/api/v1/admin/learning-path", get(path))
        .route(
            "/api/v1/admin/learning-path/publication",
            post(publish_path),
        )
        .route("/api/v1/admin/learning-path/order", put(order))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
impl ListQuery {
    pub fn core(self) -> Result<ContentQuery, ApiError> {
        Ok(ContentQuery {
            status: self.status.map(|s| ContentState::parse(&s)).transpose()?,
            limit: self.limit.unwrap_or(25),
            offset: self.offset.unwrap_or(0),
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SectionInput {
    id: Option<Uuid>,
    title: String,
    body: String,
    required: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LessonInput {
    title: String,
    description: String,
    sections: Vec<SectionInput>,
    position: i32,
    expected_revision: Option<i64>,
}
impl LessonInput {
    fn draft(self) -> LessonDraft {
        LessonDraft {
            title: self.title,
            description: self.description,
            sections: self
                .sections
                .into_iter()
                .map(|s| SectionDraft {
                    id: s.id.unwrap_or_else(Uuid::new_v4),
                    title: s.title,
                    body: s.body,
                    required: s.required,
                })
                .collect(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Order {
    lesson_ids: Vec<Uuid>,
    expected_revision: i64,
}
pub fn version_dto(v: Version) -> Value {
    json!({"id":v.id,"version":v.version,"published_at":v.published_at})
}
pub fn editor_dto(l: LessonEditor) -> Value {
    json!({"id":l.id,"state":l.state.as_str(),"revision":l.revision,"position":l.position,"draft":{"title":l.draft.title,"description":l.draft.description,"sections":l.draft.sections.into_iter().map(|s|json!({"id":s.id,"title":s.title,"body":s.body,"required":s.required})).collect::<Vec<_>>()},"versions":l.versions.into_iter().map(version_dto).collect::<Vec<_>>()})
}
pub fn path_dto(p: PathEditor) -> Value {
    json!({"id":p.id,"revision":p.revision,"lesson_ids":p.lesson_ids,"paired_bank_id":p.paired_bank_id,"quiz_bank_id":p.quiz_bank_id,"versions":p.versions.into_iter().map(version_dto).collect::<Vec<_>>(),"published":p.published.map(|m|json!({"id":m.id,"version":m.version,"paired_bank_version_id":m.paired_bank_version_id,"quiz_bank_version_id":m.quiz_bank_version_id,"lessons":m.lessons.into_iter().map(|(id,v,pos)|json!({"lesson_id":id,"lesson_version_id":v,"position":pos})).collect::<Vec<_>>()}))})
}
fn audit(a: &Authenticated) -> Result<crate::modules::identity::application::AuditActor, ApiError> {
    Ok(actor(
        &a.principal,
        a.session_id,
        REQUEST_ID.with(Clone::clone),
    )?)
}
async fn list(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiQuery(q): ApiQuery<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let q = q.core()?;
    let (limit, offset) = (q.limit, q.offset);
    let p = s.lesson_cms.list(&a.principal, q).await?;
    Ok(Json(
        json!({"data":p.rows.into_iter().map(editor_dto).collect::<Vec<_>>(),"meta":{"limit":limit,"offset":offset,"total":p.total}}),
    ))
}
async fn detail(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":editor_dto(s.lesson_cms.get(&a.principal,id).await?)}),
    ))
}
async fn create(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiJson(i): ApiJson<LessonInput>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if i.expected_revision.is_some() {
        return Err(AppError::Invalid("INVALID_INPUT").into());
    }
    let pos = i.position;
    let l = s
        .lesson_cms
        .create(
            &a.principal,
            a.session_id,
            REQUEST_ID.with(Clone::clone),
            i.draft(),
            pos,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(json!({"data":editor_dto(l)}))))
}
async fn update(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(i): ApiJson<LessonInput>,
) -> Result<Json<Value>, ApiError> {
    let rev = i
        .expected_revision
        .ok_or(AppError::Invalid("REVISION_REQUIRED"))?;
    let pos = i.position;
    Ok(Json(
        json!({"data":editor_dto(s.lesson_cms.update(&a.principal,audit(&a)?,id,i.draft(),pos,rev).await?)}),
    ))
}
async fn publish(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(i): ApiJson<Revision>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":editor_dto(s.lesson_cms.publish(&a.principal,audit(&a)?,id,i.expected_revision).await?)}),
    ))
}
async fn archive(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(i): ApiJson<Revision>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":editor_dto(s.lesson_cms.archive(&a.principal,audit(&a)?,id,i.expected_revision).await?)}),
    ))
}
pub async fn publish_path(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiJson(i): ApiJson<Revision>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":path_dto(s.lesson_cms.publish_path(&a.principal,audit(&a)?,i.expected_revision).await?)}),
    ))
}
async fn path(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":path_dto(s.lesson_cms.path(&a.principal).await?)}),
    ))
}
async fn order(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiJson(i): ApiJson<Order>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":path_dto(s.lesson_cms.order(&a.principal,audit(&a)?,i.lesson_ids,i.expected_revision).await?)}),
    ))
}

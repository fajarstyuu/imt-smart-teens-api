//! Admin-only allowlisted editor DTOs. No persistence or SQL is reachable from handlers.
mod dto;
use super::application::*;
use crate::transport::http::editor::{Data, ListQuery, PageDto, PageMeta, Revision};
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
use dto::{LessonEditorDto, PathEditorDto, editor_dto, path_dto};
use serde::Deserialize;
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
        .route("/api/v1/admin/learning-path/release", post(release))
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
struct Order {
    lesson_ids: Vec<Uuid>,
    expected_revision: i64,
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
) -> Result<Json<PageDto<LessonEditorDto>>, ApiError> {
    let q = q.core()?;
    let (limit, offset) = (q.limit, q.offset);
    let p = s.lesson_cms.list(&a.principal, q).await?;
    Ok(Json(PageDto {
        data: p.rows.into_iter().map(editor_dto).collect(),
        meta: PageMeta {
            limit,
            offset,
            total: p.total,
        },
    }))
}
async fn detail(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Data<LessonEditorDto>>, ApiError> {
    Ok(Json(Data {
        data: editor_dto(s.lesson_cms.get(&a.principal, id).await?),
    }))
}
async fn create(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiJson(i): ApiJson<LessonInput>,
) -> Result<(StatusCode, Json<Data<LessonEditorDto>>), ApiError> {
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
    Ok((
        StatusCode::CREATED,
        Json(Data {
            data: editor_dto(l),
        }),
    ))
}
async fn update(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(i): ApiJson<LessonInput>,
) -> Result<Json<Data<LessonEditorDto>>, ApiError> {
    let rev = i
        .expected_revision
        .ok_or(AppError::Invalid("REVISION_REQUIRED"))?;
    let pos = i.position;
    Ok(Json(Data {
        data: editor_dto(
            s.lesson_cms
                .update(&a.principal, audit(&a)?, id, i.draft(), pos, rev)
                .await?,
        ),
    }))
}
async fn publish(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(i): ApiJson<Revision>,
) -> Result<Json<Data<LessonEditorDto>>, ApiError> {
    Ok(Json(Data {
        data: editor_dto(
            s.lesson_cms
                .publish(&a.principal, audit(&a)?, id, i.expected_revision)
                .await?,
        ),
    }))
}
async fn archive(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(i): ApiJson<Revision>,
) -> Result<Json<Data<LessonEditorDto>>, ApiError> {
    Ok(Json(Data {
        data: editor_dto(
            s.lesson_cms
                .archive(&a.principal, audit(&a)?, id, i.expected_revision)
                .await?,
        ),
    }))
}
pub async fn publish_path(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiJson(i): ApiJson<Revision>,
) -> Result<Json<Data<PathEditorDto>>, ApiError> {
    Ok(Json(Data {
        data: path_dto(
            s.lesson_cms
                .publish_path(&a.principal, audit(&a)?, i.expected_revision)
                .await?,
        ),
    }))
}
async fn path(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
) -> Result<Json<Data<PathEditorDto>>, ApiError> {
    Ok(Json(Data {
        data: path_dto(s.lesson_cms.path(&a.principal).await?),
    }))
}
async fn order(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiJson(i): ApiJson<Order>,
) -> Result<Json<Data<PathEditorDto>>, ApiError> {
    Ok(Json(Data {
        data: path_dto(
            s.lesson_cms
                .order(&a.principal, audit(&a)?, i.lesson_ids, i.expected_revision)
                .await?,
        ),
    }))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedRootInput {
    id: Uuid,
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseInput {
    expected_revision: i64,
    lessons: Vec<ReviewedRootInput>,
    banks: Vec<ReviewedRootInput>,
}
async fn release(
    State(s): State<AppState>,
    AdminSession(a): AdminSession,
    ApiJson(input): ApiJson<ReleaseInput>,
) -> Result<Json<Data<PathEditorDto>>, ApiError> {
    let map = |r: ReviewedRootInput| ReviewedRoot {
        id: r.id,
        expected_revision: r.expected_revision,
    };
    let command = ReviewedRelease {
        expected_revision: input.expected_revision,
        lessons: input.lessons.into_iter().map(map).collect(),
        banks: input.banks.into_iter().map(map).collect(),
    };
    Ok(Json(Data {
        data: path_dto(
            s.lesson_cms
                .release(&a.principal, audit(&a)?, command)
                .await?,
        ),
    }))
}

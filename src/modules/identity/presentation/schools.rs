//! Catalog handlers translate DTOs and delegate all authorization/mutation policies.
use super::dto::*;
use crate::transport::http::{
    AppState,
    error::{ApiError, REQUEST_ID},
    extractors::{AdminSession, ApiJson, ApiPath, ApiQuery},
};
use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;
pub async fn public_schools(
    State(state): State<AppState>,
    ApiQuery(query): ApiQuery<SchoolListQuery>,
) -> Result<Json<Value>, ApiError> {
    let query = query.into_query(true)?;
    let (limit, offset) = (query.limit, query.offset);
    let page = state.schools.public_list(query).await?;
    Ok(Json(
        json!({"data":page.schools.into_iter().map(PublicSchoolDto::from).collect::<Vec<_>>(),"meta":{"limit":limit,"offset":offset,"total":page.total}}),
    ))
}
pub async fn admin_schools(
    State(state): State<AppState>,
    AdminSession(auth): AdminSession,
    ApiQuery(query): ApiQuery<SchoolListQuery>,
) -> Result<Json<Value>, ApiError> {
    let query = query.into_query(false)?;
    let (limit, offset) = (query.limit, query.offset);
    let page = state.schools.list(&auth.principal, query).await?;
    Ok(Json(
        json!({"data":page.schools.into_iter().map(SchoolDto::from).collect::<Vec<_>>(),"meta":{"limit":limit,"offset":offset,"total":page.total}}),
    ))
}
pub async fn school_detail(
    State(state): State<AppState>,
    AdminSession(auth): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({"data":SchoolDto::from(state.schools.get(&auth.principal,id).await?)}),
    ))
}
pub async fn create_school(
    State(state): State<AppState>,
    AdminSession(auth): AdminSession,
    ApiJson(input): ApiJson<CreateSchool>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let school = state
        .schools
        .create(
            &auth.principal,
            auth.session_id,
            REQUEST_ID.with(Clone::clone),
            &input.name,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"data":SchoolDto::from(school)})),
    ))
}
pub async fn rename_school(
    State(state): State<AppState>,
    AdminSession(auth): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(input): ApiJson<RenameSchool>,
) -> Result<Json<Value>, ApiError> {
    let school = state
        .schools
        .rename(
            &auth.principal,
            auth.session_id,
            REQUEST_ID.with(Clone::clone),
            id,
            &input.name,
            input.expected_revision,
        )
        .await?;
    Ok(Json(json!({"data":SchoolDto::from(school)})))
}
pub async fn archive_school(
    State(state): State<AppState>,
    AdminSession(auth): AdminSession,
    ApiPath(id): ApiPath<Uuid>,
    ApiQuery(input): ApiQuery<ArchiveSchool>,
) -> Result<Json<Value>, ApiError> {
    let school = state
        .schools
        .archive(
            &auth.principal,
            auth.session_id,
            REQUEST_ID.with(Clone::clone),
            id,
            input.expected_revision,
        )
        .await?;
    Ok(Json(
        json!({"data":{"id":school.id,"status":"archived","archived_at":school.archived_at,"revision":school.revision}}),
    ))
}

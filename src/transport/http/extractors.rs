//! Owned session principals and normalized extractor rejections for every feature.
use super::{csrf::cookie, error::ApiError, state::AppState};
use crate::{modules::identity::application::Authenticated, shared::application::AppError};
use axum::{
    extract::{FromRequest, FromRequestParts, Json, Path, Query, Request},
    http::request::Parts,
};
use serde::de::DeserializeOwned;
use subtle::ConstantTimeEq;

pub struct AdminSession(pub Authenticated);
impl FromRequestParts<AppState> for AdminSession {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = authenticate(parts, state, "imt_admin_session").await?;
        auth.principal.admin()?;
        Ok(Self(auth))
    }
}
/// Student workflows reuse this extractor and still enforce resource ownership in use cases.
pub struct StudentSession(pub Authenticated);
impl FromRequestParts<AppState> for StudentSession {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = authenticate(parts, state, "imt_student_session").await?;
        auth.principal.student()?;
        Ok(Self(auth))
    }
}
async fn authenticate(
    parts: &mut Parts,
    state: &AppState,
    name: &str,
) -> Result<Authenticated, ApiError> {
    let token = cookie(&parts.headers, name).ok_or(AppError::Unauthorized)?;
    let auth = state.sessions.authenticate(token).await?;
    if !matches!(
        parts.method,
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) {
        let binding = state.csrf.validate(&parts.headers)?;
        if !bool::from(binding.ct_eq(&auth.csrf_binding_hash)) {
            return Err(AppError::Forbidden.into());
        }
    }
    Ok(auth)
}
pub struct ApiJson<T>(pub T);
impl<T: DeserializeOwned + Send> FromRequest<AppState> for ApiJson<T> {
    type Rejection = ApiError;
    async fn from_request(req: Request, state: &AppState) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(v)| Self(v))
            .map_err(|e| {
                ApiError::transport(
                    e.status(),
                    match e.status() {
                        axum::http::StatusCode::UNPROCESSABLE_ENTITY => "INVALID_INPUT",
                        axum::http::StatusCode::PAYLOAD_TOO_LARGE => "BODY_TOO_LARGE",
                        axum::http::StatusCode::UNSUPPORTED_MEDIA_TYPE => "UNSUPPORTED_MEDIA_TYPE",
                        _ => "INVALID_JSON",
                    },
                )
            })
    }
}
pub struct ApiQuery<T>(pub T);
impl<T: DeserializeOwned + Send> FromRequestParts<AppState> for ApiQuery<T> {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(v)| Self(v))
            .map_err(|_| ApiError::transport(axum::http::StatusCode::BAD_REQUEST, "INVALID_QUERY"))
    }
}
pub struct ApiPath<T>(pub T);
impl<T: DeserializeOwned + Send> FromRequestParts<AppState> for ApiPath<T> {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(v)| Self(v))
            .map_err(|_| ApiError::transport(axum::http::StatusCode::BAD_REQUEST, "INVALID_PATH"))
    }
}

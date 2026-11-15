//! Thin admin-session endpoints. Cookies are issued only after durable insertion.
use super::dto::{AdminDto, LoginRequest};
use crate::{
    shared::application::AppError,
    transport::http::{
        AppState,
        csrf::{Csrf, cookie, cookie_value},
        error::{ApiError, REQUEST_ID},
        extractors::{AdminSession, ApiJson},
    },
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;

pub async fn csrf_context(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    // Reuse a valid context to avoid breaking other tabs; bind a fresh context after expiry.
    let token = match cookie(&headers, "imt_csrf").filter(|s| state.csrf.valid(s)) {
        Some(token) => token.to_owned(),
        None => state.csrf.issue()?,
    };
    let binding = Csrf::binding(&token);
    for name in ["imt_admin_session", "imt_student_session"] {
        if let Some(token) = cookie(&headers, name) {
            match state.sessions.authenticate(token).await {
                Ok(auth) => state.sessions.bind_csrf(auth.session_id, &binding).await?,
                Err(AppError::Unauthorized) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    let mut response = Json(json!({"data":{"token":token}})).into_response();
    response
        .headers_mut()
        .append(header::SET_COOKIE, state.csrf.cookie(&token)?);
    // Stable, signed browser-operation scope survives login CSRF rotations. It is not authentication.
    let browser = cookie(&headers, "imt_enrollment_context")
        .filter(|s| state.csrf.valid(s))
        .unwrap_or(&token);
    response.headers_mut().append(
        header::SET_COOKIE,
        cookie_value(
            "imt_enrollment_context",
            browser,
            7200,
            state.settings.session.secure,
        )?,
    );
    Ok(response)
}
pub async fn admin_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<LoginRequest>,
) -> Result<Response, ApiError> {
    state.csrf.validate(&headers)?;
    let next_context = state.csrf.issue()?;
    let request_id = REQUEST_ID.with(Clone::clone);
    let (admin, issued) = state
        .identity
        .login(
            &input.email,
            input.password,
            crate::modules::identity::application::AdminLoginContext {
                binding: Csrf::binding(&next_context),
                previous_admin_token: cookie(&headers, "imt_admin_session").map(str::to_owned),
                student_token: cookie(&headers, "imt_student_session").map(str::to_owned),
                request_id,
            },
        )
        .await?;
    let mut response = (
        StatusCode::CREATED,
        Json(json!({"data":{"admin":AdminDto::from(admin),"csrf_token":next_context}})),
    )
        .into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        cookie_value(
            "imt_admin_session",
            &issued.token,
            state.settings.session.admin_absolute_seconds,
            state.settings.session.secure,
        )?,
    );
    response
        .headers_mut()
        .append(header::SET_COOKIE, state.csrf.cookie(&next_context)?);
    Ok(response)
}
pub async fn admin_me(
    AdminSession(auth): AdminSession,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(
        json!({"data":{"admin":AdminDto::from(auth.principal.admin()?.clone())}}),
    ))
}
pub async fn admin_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    // Logout is idempotent after revocation, but a still-live session must have its
    // binding checked so an unrelated valid CSRF context cannot log it out.
    let binding = state.csrf.validate(&headers)?;
    if let Some(token) = cookie(&headers, "imt_admin_session") {
        match state.sessions.authenticate(token).await {
            Ok(auth) => {
                use subtle::ConstantTimeEq;
                auth.principal.admin()?;
                if !bool::from(binding.ct_eq(&auth.csrf_binding_hash)) {
                    return Err(AppError::Forbidden.into());
                }
            }
            Err(AppError::Unauthorized) => {}
            Err(error) => return Err(error.into()),
        }
    }
    state
        .sessions
        .revoke(
            cookie(&headers, "imt_admin_session"),
            &REQUEST_ID.with(Clone::clone),
        )
        .await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        cookie_value("imt_admin_session", "", 0, state.settings.session.secure)?,
    );
    Ok(response)
}

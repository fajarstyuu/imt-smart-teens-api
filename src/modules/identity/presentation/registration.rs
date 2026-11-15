//! Student browser-session transport; secrets are set only as cookies after commit.
use crate::{
    modules::{
        identity::{
            application::{Registration, RegistrationContext, TokenPort},
            infrastructure::SecureTokens,
        },
        learning::application::Operation,
    },
    shared::error::AppError,
    transport::http::{
        AppState,
        csrf::{Csrf, cookie, cookie_value},
        error::{ApiError, REQUEST_ID},
        extractors::ApiJson,
    },
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::json;
/// HTTP-only shape separates malformed UUID syntax (400) from missing/invalid values (422).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrationRequest {
    pub name: String,
    pub school_id: String,
    pub form_date: String,
}
pub async fn student_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(input): ApiJson<RegistrationRequest>,
) -> Result<Response, ApiError> {
    state.csrf.validate(&headers)?;
    let input = Registration {
        name: input.name,
        school_id: input
            .school_id
            .parse()
            .map_err(|_| ApiError::transport(StatusCode::BAD_REQUEST, "INVALID_SCHOOL_ID"))?,
        form_date: input.form_date,
    };
    // Established before enrollment, so response loss and CSRF rotation retain the operation scope.
    let browser = cookie(&headers, "imt_enrollment_context")
        .filter(|s| state.csrf.valid(s))
        .ok_or(AppError::Forbidden)?;
    let next = state.csrf.issue()?;
    let op = Operation::new(
        Csrf::binding(browser),
        headers
            .get("idempotency-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned(),
        &input,
        REQUEST_ID.with(Clone::clone),
    )?;
    let (mut out, issued) = state
        .students
        .register(
            input,
            RegistrationContext {
                operation: op,
                binding: Csrf::binding(&next),
                previous_student_hash: cookie(&headers, "imt_student_session")
                    .map(|s| SecureTokens.digest(s)),
                admin_hash: cookie(&headers, "imt_admin_session").map(|s| SecureTokens.digest(s)),
            },
        )
        .await?;
    out["csrf_token"] = json!(next);
    let mut response = (StatusCode::CREATED, Json(json!({"data":out}))).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        cookie_value(
            "imt_student_session",
            &issued.token,
            state.settings.session.student_absolute_seconds,
            state.settings.session.secure,
        )?,
    );
    response
        .headers_mut()
        .append(header::SET_COOKIE, state.csrf.cookie(&next)?);
    Ok(response)
}
pub async fn student_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    use subtle::ConstantTimeEq;
    let binding = state.csrf.validate(&headers)?;
    if let Some(token) = cookie(&headers, "imt_student_session") {
        match state.sessions.authenticate(token).await {
            Ok(auth) => {
                auth.principal.student()?;
                if !bool::from(binding.ct_eq(&auth.csrf_binding_hash)) {
                    return Err(AppError::Forbidden.into());
                }
            }
            Err(AppError::Unauthorized) => {}
            Err(e) => return Err(e.into()),
        }
    }
    state
        .sessions
        .revoke(
            cookie(&headers, "imt_student_session"),
            &REQUEST_ID.with(Clone::clone),
        )
        .await?;
    let mut out = StatusCode::NO_CONTENT.into_response();
    out.headers_mut().append(
        header::SET_COOKIE,
        cookie_value("imt_student_session", "", 0, state.settings.session.secure)?,
    );
    Ok(out)
}

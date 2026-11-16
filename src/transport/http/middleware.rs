//! Request decoration surrounds CORS; policy and cancellation run inside CORS.
use super::{
    AppState,
    error::{ApiError, REQUEST_ID},
};
use crate::shared::application::AppError;
use axum::{
    extract::{ConnectInfo, MatchedPath, Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Instant,
};

/// Outermost scope decorates every response, including CORS preflights and
/// policy/deadline failures. Error mapping reads the same task-local ID.
pub async fn common(request: Request, next: Next) -> Response {
    let id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let started = Instant::now();
    let method = request.method().clone();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str())
        .unwrap_or("unmatched")
        .to_owned();
    REQUEST_ID
        .scope(id.clone(), async move {
            let mut response = next.run(request).await;
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            if let Ok(value) = HeaderValue::from_str(&id) {
                response.headers_mut().insert("x-request-id", value);
            }
            response.headers_mut().insert(
                "x-content-type-options",
                HeaderValue::from_static("nosniff"),
            );
            tracing::info!(
                request_id = %id,
                method = %method,
                route = %route,
                status = response.status().as_u16(),
                elapsed_ms = started.elapsed().as_millis(),
                "request completed"
            );
            response
        })
        .await
}

/// CORS must wrap this layer: all early returns are normal structured responses
/// that an allowlisted browser needs permission to read. Dropping `next.run`
/// cancels application work/transactions, rather than detaching business tasks.
pub async fn policy(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if let Err(error) = check_policy(&state, &request) {
        return error.into_response();
    }
    tokio::select! {
        biased;
        _ = state.requests.cancelled() => ApiError::from(AppError::Unavailable).into_response(),
        result = tokio::time::timeout(state.settings.http.request_timeout, next.run(request)) => {
            match result {
                Ok(response) => response,
                Err(_) => ApiError::transport(StatusCode::GATEWAY_TIMEOUT, "REQUEST_TIMEOUT").into_response(),
            }
        }
    }
}

fn check_policy(state: &AppState, request: &Request) -> Result<(), ApiError> {
    let method = request.method();
    let path = request.uri().path();
    let ip = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|i| i.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let ip = super::client_ip::resolve(
        ip,
        request.headers(),
        &state.settings.http.trusted_proxy_ips,
    );
    let login = method == Method::POST && path == "/api/v1/admin/sessions";
    if !path.starts_with("/health/") && method != Method::OPTIONS && !state.limiter.allow(ip, login)
    {
        return Err(ApiError::transport(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
        ));
    }
    let content_length = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok());
    if content_length.is_some_and(|length| length > state.settings.http.body_limit) {
        return Err(ApiError::transport(
            StatusCode::PAYLOAD_TOO_LARGE,
            "BODY_TOO_LARGE",
        ));
    }
    if path.starts_with("/api/v1/")
        && !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
    {
        state.csrf.validate(request.headers())?;
    }
    Ok(())
}

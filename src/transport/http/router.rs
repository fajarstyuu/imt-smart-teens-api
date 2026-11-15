//! Explicit v1 route inventory; unassigned workflows are deliberately not registered.
use super::{AppState, error::ApiError, middleware};
use crate::{modules::identity::presentation::*, shared::application::AppError};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::{HeaderValue, Method, StatusCode},
    middleware as axum_middleware,
    routing::{delete, get, post},
};
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;
pub fn build_router(state: AppState) -> Router {
    let origins = state
        .settings
        .http
        .allowed_origins
        .iter()
        .filter_map(|o| o.parse::<HeaderValue>().ok())
        .collect::<Vec<_>>();
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderName::from_static("x-csrf-token"),
            axum::http::HeaderName::from_static("idempotency-key"),
        ])
        .expose_headers([axum::http::HeaderName::from_static("x-request-id")]);
    Router::new()
        .merge(crate::modules::learning::admin::presentation::routes())
        .merge(crate::modules::learning::presentation::routes())
        .route(
            "/health/live",
            get(|| async { Json(json!({"data":{"status":"live"}})) }),
        )
        .route("/health/ready", get(ready))
        .route("/api/v1/config", get(config))
        .route("/api/v1/csrf", get(csrf_context))
        .route("/api/v1/admin/sessions", post(admin_login))
        .route("/api/v1/admin/sessions/current", delete(admin_logout))
        .route("/api/v1/admin/me", get(admin_me))
        .route("/api/v1/schools", get(public_schools))
        .route(
            "/api/v1/admin/schools",
            get(admin_schools).post(create_school),
        )
        .route(
            "/api/v1/admin/schools/{school_id}",
            get(school_detail).put(rename_school).delete(archive_school),
        )
        .fallback(|| async { ApiError::transport(StatusCode::NOT_FOUND, "NOT_FOUND") })
        .method_not_allowed_fallback(|| async {
            ApiError::transport(StatusCode::METHOD_NOT_ALLOWED, "METHOD_NOT_ALLOWED")
        })
        // Incoming order (last layer is outermost): correlation/security ->
        // CORS -> policy/cancellation/deadline -> body limit -> handlers.
        // CORS therefore also decorates early policy and timeout responses.
        .layer(DefaultBodyLimit::max(state.settings.http.body_limit))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::policy,
        ))
        .layer(cors)
        .layer(axum_middleware::from_fn(middleware::common))
        .with_state(state)
}
async fn ready(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    if state.readiness.check().await {
        Ok(Json(json!({"data":{"status":"ready"}})))
    } else {
        Err(AppError::Unavailable.into())
    }
}
async fn config(State(state): State<AppState>) -> Json<Value> {
    Json(
        json!({"data":{"brand":"Imt Smart Teens","program":"IMT","environment":state.settings.environment}}),
    )
}

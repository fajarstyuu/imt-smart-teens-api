//! Requests exercise our configured limits, origin policy, rejection mapping and CORS order.
mod common;
use axum::{
    Router,
    body::Body,
    http::{Request, header},
};
use common::*;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
async fn raw(
    app: &Router,
    method: &str,
    path: &str,
    body: String,
    headers: &[(&str, &str)],
) -> (u16, Value, axum::http::HeaderMap) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    if status >= 400 {
        assert_eq!(
            body["request_id"].as_str().unwrap(),
            headers["x-request-id"]
        );
        assert!(body["error"]["message"].as_str().unwrap().contains(' '));
    }
    (status, body, headers)
}
/// A browser must be able to read policy errors, not just successful handlers.
fn assert_browser_error(status: u16, body: &Value, headers: &axum::http::HeaderMap, code: &str) {
    assert!(status >= 400);
    assert_eq!(body["error"]["code"], code);
    assert_eq!(
        body["request_id"].as_str().unwrap(),
        headers["x-request-id"]
    );
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        headers
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok()),
        Some("http://localhost:5173")
    );
    assert_eq!(headers[header::ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
}

#[sqlx::test]
async fn allowed_origin_can_read_csrf_policy_errors(pool: PgPool) {
    let app = app(pool).await;
    let (s, b, h) = raw(
        &app,
        "POST",
        "/api/v1/admin/schools",
        "{}".into(),
        &[("origin", "http://localhost:5173")],
    )
    .await;
    assert_eq!(s, 403);
    assert_browser_error(s, &b, &h, "FORBIDDEN");
    let (s, _, h) = raw(
        &app,
        "POST",
        "/api/v1/admin/schools",
        "{}".into(),
        &[("origin", "https://evil.invalid")],
    )
    .await;
    assert_eq!(s, 403);
    assert!(!h.contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
}

#[sqlx::test]
async fn allowed_origin_can_read_body_policy_errors(pool: PgPool) {
    let app = app(pool).await;
    let (s, b, h) = raw(
        &app,
        "POST",
        "/api/v1/admin/schools",
        String::new(),
        &[
            ("origin", "http://localhost:5173"),
            ("content-length", "70000"),
        ],
    )
    .await;
    assert_eq!(s, 413);
    assert_browser_error(s, &b, &h, "BODY_TOO_LARGE");
}

#[sqlx::test]
async fn allowed_origin_can_read_rate_policy_errors(pool: PgPool) {
    let app = app(pool).await;
    // Missing CSRF is cheap but still consumes the real login limiter.
    for _ in 0..10 {
        assert_eq!(
            raw(
                &app,
                "POST",
                "/api/v1/admin/sessions",
                String::new(),
                &[("origin", "http://localhost:5173")]
            )
            .await
            .0,
            403
        );
    }
    let (s, b, h) = raw(
        &app,
        "POST",
        "/api/v1/admin/sessions",
        String::new(),
        &[("origin", "http://localhost:5173")],
    )
    .await;
    assert_eq!(s, 429);
    assert_browser_error(s, &b, &h, "RATE_LIMITED");
}

#[sqlx::test]
async fn malformed_json_media_body_path_and_query_rejections_use_envelope(pool: PgPool) {
    let id = admin(&pool, true).await;
    let app = app(pool).await;
    let (c, t) = login(&app, id).await;
    let headers = [
        ("origin", "http://localhost:5173"),
        ("cookie", c.as_str()),
        ("x-csrf-token", t.as_str()),
        ("content-type", "application/json"),
    ];
    assert_eq!(
        raw(&app, "POST", "/api/v1/admin/schools", "{".into(), &headers)
            .await
            .0,
        400
    );
    assert_eq!(
        raw(&app, "POST", "/api/v1/admin/schools", "{}".into(), &headers)
            .await
            .0,
        422
    );
    assert_eq!(
        raw(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"A","role":"admin"}).to_string(),
            &headers
        )
        .await
        .0,
        422
    );
    assert_eq!(
        raw(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"x".repeat(70000)}).to_string(),
            &headers
        )
        .await
        .0,
        413
    );
    assert_eq!(
        raw(
            &app,
            "POST",
            "/api/v1/admin/schools",
            "{}".into(),
            &headers[..3]
        )
        .await
        .0,
        415
    );
    assert_eq!(
        raw(
            &app,
            "GET",
            "/api/v1/admin/schools/not-a-uuid",
            String::new(),
            &headers
        )
        .await
        .0,
        400
    );
    assert_eq!(
        raw(
            &app,
            "GET",
            "/api/v1/schools?limit=oops",
            String::new(),
            &headers
        )
        .await
        .0,
        400
    );
    let (_, v, h) = raw(
        &app,
        "GET",
        "/missing",
        String::new(),
        &[("x-request-id", "safe-request-id")],
    )
    .await;
    assert_eq!(h["x-request-id"], "safe-request-id");
    assert_eq!(v["error"]["code"], "NOT_FOUND");
}
#[sqlx::test]
async fn origin_and_csrf_are_mandatory_and_preflight_does_not_need_auth(pool: PgPool) {
    let id = admin(&pool, true).await;
    let app = app(pool).await;
    let (c, t) = login(&app, id).await;
    let headers = [
        ("origin", "https://evil.invalid"),
        ("cookie", c.as_str()),
        ("x-csrf-token", t.as_str()),
        ("content-type", "application/json"),
    ];
    assert_eq!(
        raw(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"A"}).to_string(),
            &headers
        )
        .await
        .0,
        403
    );
    assert_eq!(
        raw(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"A"}).to_string(),
            &headers[1..]
        )
        .await
        .0,
        403
    );
    let (s, _, h) = raw(
        &app,
        "OPTIONS",
        "/api/v1/admin/schools",
        String::new(),
        &[
            ("origin", "http://localhost:5173"),
            ("access-control-request-method", "POST"),
            (
                "access-control-request-headers",
                "content-type,x-csrf-token",
            ),
        ],
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(
        h[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "http://localhost:5173"
    );
    assert_eq!(h[header::ACCESS_CONTROL_ALLOW_CREDENTIALS], "true");
}
#[sqlx::test]
async fn login_rate_limit_is_real_and_does_not_block_liveness(pool: PgPool) {
    let app = app(pool).await;
    let (c, t) = csrf(&app).await;
    for _ in 0..10 {
        assert_eq!(
            request(
                &app,
                "POST",
                "/api/v1/admin/sessions",
                json!({"email":"absent@example.invalid","password":"synthetic-invalid-password"}),
                &c,
                &t
            )
            .await
            .0,
            401
        );
    }
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/sessions",
            json!({"email":"absent@example.invalid","password":"synthetic-invalid-password"}),
            &c,
            &t
        )
        .await
        .0,
        429
    );
    assert_eq!(
        request(&app, "GET", "/health/live", json!(null), "", "")
            .await
            .0,
        200
    );
}

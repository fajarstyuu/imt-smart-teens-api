//! Synthetic fixtures live here, never in production constructors.
// Each integration-test target compiles its own subset of these shared fixtures.
#![allow(dead_code)]
use axum::{
    Router,
    body::Body,
    http::{Request, header},
};
use http_body_util::BodyExt;
use imt_smart_teens_backend::{
    bootstrap::assemble, config::Settings, transport::http::build_router,
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

pub fn settings() -> Settings {
    Settings::from_lookup(|key| match key {
        "DATABASE_URL" => Some("postgres://unused@127.0.0.1:1/unused".to_owned()),
        "CSRF_SIGNING_KEY" => Some("synthetic-test-key-not-for-deployment-000000000000".to_owned()),
        "CORS_ALLOWED_ORIGINS" => Some("http://localhost:5173".to_owned()),
        _ => None,
    })
    .unwrap()
}
pub async fn app(pool: PgPool) -> Router {
    build_router(assemble(pool, settings()).await.unwrap())
}
pub async fn request(
    app: &Router,
    method: &str,
    path: &str,
    body: Value,
    cookie: &str,
    token: &str,
) -> (u16, Value, Vec<String>) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, "http://localhost:5173");
    if !cookie.is_empty() {
        req = req.header(header::COOKIE, cookie);
    }
    if !token.is_empty() {
        req = req.header("x-csrf-token", token);
    }
    let response = app
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(response.headers().contains_key("x-request-id"));
    let status = response.status().as_u16();
    let cookies = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap().to_owned())
        .collect();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    if status >= 400 {
        assert!(value["error"]["code"].is_string());
        assert!(value["request_id"].is_string());
    }
    (status, value, cookies)
}
pub fn cookie_header(cookies: &[String]) -> String {
    cookies
        .iter()
        .map(|s| s.split(';').next().unwrap())
        .collect::<Vec<_>>()
        .join("; ")
}
pub async fn csrf(app: &Router) -> (String, String) {
    let (status, value, cookies) = request(app, "GET", "/api/v1/csrf", json!(null), "", "").await;
    assert_eq!(status, 200);
    (
        cookie_header(&cookies),
        value["data"]["token"].as_str().unwrap().to_owned(),
    )
}
pub async fn admin(pool: &PgPool, manage: bool) -> uuid::Uuid {
    use imt_smart_teens_backend::modules::identity::{
        application::PasswordPort, infrastructure::ArgonPasswords,
    };
    let hash = ArgonPasswords::new(2)
        .hash("synthetic-only-password-234".to_owned())
        .await
        .unwrap();
    let id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO identity_admins(id,email,display_name,password_hash,can_manage_schools) VALUES($1,$2,'Synthetic admin',$3,$4)").bind(id).bind(format!("{id}@example.invalid")).bind(hash).bind(manage).execute(pool).await.unwrap();
    id
}
pub async fn login(app: &Router, id: uuid::Uuid) -> (String, String) {
    let (cookie, token) = csrf(app).await;
    let (status, value, cookies) = request(
        app,
        "POST",
        "/api/v1/admin/sessions",
        json!({"email":format!("{id}@example.invalid"),"password":"synthetic-only-password-234"}),
        &cookie,
        &token,
    )
    .await;
    assert_eq!(status, 201, "{value}");
    (
        cookie_header(&cookies),
        value["data"]["csrf_token"].as_str().unwrap().to_owned(),
    )
}

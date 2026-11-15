//! Reusable real-router student requests for CMS and reporting integration tests.
#![allow(dead_code)]
use crate::common::*;
use axum::{
    Router,
    body::Body,
    http::{Request, header},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;
pub async fn call(
    app: &Router,
    method: &str,
    path: &str,
    body: Value,
    cookie: &str,
    token: &str,
    key: &str,
) -> (u16, Value, Vec<String>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::ORIGIN, "http://localhost:5173")
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", token)
                .header("idempotency-key", key)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
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
    (status, value, cookies)
}
pub fn merge_cookie(old: &str, new: &[String]) -> String {
    let mut jar = std::collections::BTreeMap::new();
    for part in old
        .split(';')
        .map(str::trim)
        .chain(new.iter().map(|s| s.split(';').next().unwrap()))
    {
        if let Some((k, v)) = part.split_once('=') {
            jar.insert(k.to_owned(), v.to_owned());
        }
    }
    jar.into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}
pub async fn enroll(app: &Router, school: Uuid) -> (String, String, Value) {
    let (cookie, token) = csrf(app).await;
    let (s, v, c) = call(
        app,
        "POST",
        "/api/v1/student/sessions",
        json!({"name":"Synthetic student","school_id":school,"form_date":"2026-11-15"}),
        &cookie,
        &token,
        "enroll",
    )
    .await;
    assert_eq!(s, 201, "{v}");
    (
        merge_cookie(&cookie, &c),
        v["data"]["csrf_token"].as_str().unwrap().into(),
        v["data"].clone(),
    )
}
pub async fn attempt(app: &Router, c: &str, t: &str, typ: &str) -> Value {
    let (s, v, _) = call(
        app,
        "POST",
        "/api/v1/student/attempts",
        json!({"assessment_type":typ}),
        c,
        t,
        typ,
    )
    .await;
    assert!([200, 201].contains(&s), "{s}: {v}");
    let id = v["data"]["attempt_id"].as_str().unwrap();
    let (s, v, _) = call(
        app,
        "GET",
        &format!("/api/v1/student/attempts/{id}"),
        json!(null),
        c,
        t,
        "",
    )
    .await;
    assert_eq!(s, 200, "{v}");
    v["data"].clone()
}
pub async fn submit(app: &Router, c: &str, t: &str, a: &Value, key: &str) -> Value {
    let (s, v, _) = call(
        app,
        "POST",
        &format!(
            "/api/v1/student/attempts/{}/submission",
            a["id"].as_str().unwrap()
        ),
        json!({}),
        c,
        t,
        key,
    )
    .await;
    assert_eq!(s, 200, "{v}");
    v["data"].clone()
}

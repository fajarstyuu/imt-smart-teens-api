//! Real transaction cancellation/rollback tests, not mocked repository behavior.
mod common;
use common::*;
use imt_smart_teens_backend::{
    bootstrap::assemble,
    modules::identity::domain::{AdminPrincipal, Principal},
    transport::http::build_router,
};
use serde_json::json;
use sqlx::PgPool;
#[sqlx::test]
async fn request_deadline_rolls_back_blocked_school_edit(pool: PgPool) {
    let admin_id = admin(&pool, true).await;
    let mut settings = settings();
    settings.http.request_timeout = std::time::Duration::from_secs(1);
    let app = build_router(assemble(pool.clone(), settings).await.unwrap());
    let (c, t) = login(&app, admin_id).await;
    let (_, v, _) = request(
        &app,
        "POST",
        "/api/v1/admin/schools",
        json!({"name":"Before timeout"}),
        &c,
        &t,
    )
    .await;
    let id = uuid::Uuid::parse_str(v["data"]["id"].as_str().unwrap()).unwrap();
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM identity_schools WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *lock)
        .await
        .unwrap();
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/v1/admin/schools/{id}"))
                .header("origin", "http://localhost:5173")
                .header("cookie", c)
                .header("x-csrf-token", t)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name":"Must roll back","expected_revision":1}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 504);
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["error"]["code"], "REQUEST_TIMEOUT");
    assert_eq!(v["request_id"].as_str().unwrap(), headers["x-request-id"]);
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(
        headers
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("http://localhost:5173")
    );
    assert_eq!(headers["access-control-allow-credentials"], "true");
    lock.rollback().await.unwrap();
    let (name, revision): (String, i64) =
        sqlx::query_as("SELECT name,revision FROM identity_schools WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name, "Before timeout");
    assert_eq!(revision, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM platform_audit_events WHERE action='school.renamed'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}
#[sqlx::test]
async fn shutdown_cancellation_stops_existing_and_late_requests(pool: PgPool) {
    use std::{sync::atomic::Ordering, time::Duration};
    let admin_id = admin(&pool, true).await;
    let state = assemble(pool.clone(), settings()).await.unwrap();
    let app = build_router(state.clone());
    let (cookie, token) = login(&app, admin_id).await;
    let (_, value, _) = request(
        &app,
        "POST",
        "/api/v1/admin/schools",
        json!({"name":"Before cancellation"}),
        &cookie,
        &token,
    )
    .await;
    let id = uuid::Uuid::parse_str(value["data"]["id"].as_str().unwrap()).unwrap();
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM identity_schools WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_one(&mut *lock)
        .await
        .unwrap();
    let path = format!("/api/v1/admin/schools/{id}");
    let edit = request(
        &app,
        "PUT",
        &path,
        json!({"name":"Must roll back","expected_revision":1}),
        &cookie,
        &token,
    );
    tokio::pin!(edit);
    let waiting = async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND wait_event_type='Lock')").fetch_one(&pool).await.unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    };
    tokio::select! {
        result = tokio::time::timeout(Duration::from_secs(2), waiting) => result.expect("edit never reached the database lock"),
        _ = &mut edit => panic!("edit completed before shutdown cancellation"),
    }
    state.readiness.draining.store(true, Ordering::Relaxed);
    assert!(!state.readiness.check().await);
    state.requests.cancel();
    let (status, body, _) = tokio::time::timeout(Duration::from_millis(500), &mut edit)
        .await
        .expect("cancelled request outlived shutdown");
    assert_eq!(status, 503);
    assert_eq!(body["error"]["code"], "SERVICE_UNAVAILABLE");
    // Cancellation is sticky even when a request subscribes after the signal.
    let (status, _, _) = request(&app, "GET", "/api/v1/schools", json!(null), "", "").await;
    assert_eq!(status, 503);
    lock.rollback().await.unwrap();
    let (name, revision): (String, i64) =
        sqlx::query_as("SELECT name,revision FROM identity_schools WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name, "Before cancellation");
    assert_eq!(revision, 1);
}

#[sqlx::test]
async fn school_creation_rolls_back_when_audit_insert_fails(pool: PgPool) {
    let id = admin(&pool, true).await;
    let state = assemble(pool.clone(), settings()).await.unwrap();
    let principal = Principal::Admin(AdminPrincipal {
        id,
        name: "Synthetic".into(),
        email: "synthetic@example.invalid".into(),
        can_manage_schools: true,
        can_manage_content: false,
        school_ids: vec![],
    });
    // The deliberately nonexistent actor session violates the audit FK after INSERT school.
    assert!(
        state
            .schools
            .create(
                &principal,
                uuid::Uuid::new_v4(),
                "synthetic-request".into(),
                "Must roll back"
            )
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity_schools")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

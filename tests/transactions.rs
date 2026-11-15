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
    let (s, v, _) = request(
        &app,
        "PUT",
        &format!("/api/v1/admin/schools/{id}"),
        json!({"name":"Must roll back","expected_revision":1}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 504);
    assert_eq!(v["error"]["code"], "REQUEST_TIMEOUT");
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

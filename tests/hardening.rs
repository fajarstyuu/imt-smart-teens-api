//! Regression-driven hardening of cross-cutting session and health policies.
mod common;
use common::*;
use imt_smart_teens_backend::{
    bootstrap::assemble,
    modules::identity::{application::SessionSubject, infrastructure::PgIdentityStore},
    transport::http::{build_router, csrf::Csrf},
};
use serde_json::json;
use sqlx::PgPool;
fn id(n: u128) -> uuid::Uuid {
    uuid::Uuid::parse_str(&format!("00000000-0000-0000-0000-{n:012}")).unwrap()
}
#[sqlx::test]
async fn health_stays_available_when_api_rate_bucket_is_exhausted(pool: PgPool) {
    let app = app(pool).await;
    for _ in 0..300 {
        assert_eq!(
            request(&app, "GET", "/missing", json!(null), "", "")
                .await
                .0,
            404
        );
    }
    assert_eq!(
        request(&app, "GET", "/missing", json!(null), "", "")
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
    assert_eq!(
        request(&app, "GET", "/health/ready", json!(null), "", "")
            .await
            .0,
        200
    );
}
#[sqlx::test]
async fn admin_login_rotates_context_without_breaking_coexisting_student_session(pool: PgPool) {
    sqlx::raw_sql(include_str!("fixtures/relational.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let admin_id = admin(&pool, true).await;
    let state = assemble(pool.clone(), settings()).await.unwrap();
    let context = state.csrf.issue().unwrap();
    let issued = state
        .sessions
        .prepare(
            SessionSubject::Student {
                student_id: id(2),
                enrollment_id: id(18),
            },
            Csrf::binding(&context),
        )
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    PgIdentityStore::insert_in_transaction(&mut tx, &issued.record)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let app = build_router(state.clone());
    let cookie = format!("imt_student_session={}; imt_csrf={context}", issued.token);
    let(s,v,_)=request(&app,"POST","/api/v1/admin/sessions",json!({"email":format!("{admin_id}@example.invalid"),"password":"synthetic-only-password-234"}),&cookie,&context).await;
    assert_eq!(s, 201);
    let new_context = v["data"]["csrf_token"].as_str().unwrap();
    let student = state.sessions.authenticate(&issued.token).await.unwrap();
    assert_eq!(
        student.csrf_binding_hash,
        Csrf::binding(new_context),
        "independent role cookies must share the newly rotated browser CSRF context"
    );
}

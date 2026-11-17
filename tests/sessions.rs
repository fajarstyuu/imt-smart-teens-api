//! Session lifecycle and owned student principal foundations for Task 2.
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
async fn student_session_survives_school_archive_but_cannot_become_admin(pool: PgPool) {
    sqlx::raw_sql(include_str!("fixtures/relational.sql"))
        .execute(&pool)
        .await
        .unwrap();
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
    let auth = state.sessions.authenticate(&issued.token).await.unwrap();
    assert_eq!(auth.principal.student().unwrap(), (id(2), id(18)));
    sqlx::query("UPDATE identity_schools SET archived_at=now() WHERE id=$1")
        .bind(id(1))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        state
            .sessions
            .authenticate(&issued.token)
            .await
            .unwrap()
            .principal
            .student()
            .unwrap(),
        (id(2), id(18))
    );
    let app = build_router(state.clone());
    let cookie = format!("imt_admin_session={}; imt_csrf={context}", issued.token);
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/me", json!(null), &cookie, "")
            .await
            .0,
        403
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"Forbidden"}),
            &cookie,
            &context
        )
        .await
        .0,
        403
    );
    state
        .sessions
        .revoke(Some(&issued.token), "synthetic-request")
        .await
        .unwrap();
    assert!(state.sessions.authenticate(&issued.token).await.is_err());
}
#[sqlx::test]
async fn login_rotates_session_and_absolute_expiry_or_deactivation_is_enforced(pool: PgPool) {
    let id = admin(&pool, true).await;
    let app = app(pool.clone()).await;
    let (c, t) = login(&app, id).await;
    let (s, v, cookies) = request(
        &app,
        "POST",
        "/api/v1/admin/sessions",
        json!({"email":format!("{id}@example.invalid"),"password":"synthetic-only-password-234"}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 201);
    for cookie in &cookies {
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Path=/api/v1"));
        assert!(!cookie.contains("Domain="));
    }
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/me", json!(null), &c, "")
            .await
            .0,
        401
    );
    let c = cookie_header(&cookies);
    let t = v["data"]["csrf_token"].as_str().unwrap();
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/me", json!(null), &c, "")
            .await
            .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"Works"}),
            &c,
            t
        )
        .await
        .0,
        201
    );
    let hashes: Vec<Vec<u8>> = sqlx::query_scalar("SELECT token_hash FROM identity_sessions")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(hashes.len(), 2);
    assert!(hashes.iter().all(|h| h.len() == 32));
    sqlx::query("UPDATE identity_admins SET active=false WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/me", json!(null), &c, "")
            .await
            .0,
        401
    );
    sqlx::query("UPDATE identity_admins SET active=true WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE identity_sessions SET created_at=now()-interval '2 days',idle_expires_at=now()-interval '1 day',absolute_expires_at=now()-interval '1 day' WHERE admin_id=$1").bind(id).execute(&pool).await.unwrap();
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/me", json!(null), &c, "")
            .await
            .0,
        401
    );
}
#[sqlx::test]
async fn csrf_refresh_rebinds_expired_context_and_admin_is_global(pool: PgPool) {
    let id = admin(&pool, true).await;
    let app = app(pool.clone()).await;
    let (c, _) = login(&app, id).await;
    let session_cookie = c
        .split(';')
        .find(|v| v.trim().starts_with("imt_admin_session="))
        .unwrap();
    let (s, v, cookies) =
        request(&app, "GET", "/api/v1/csrf", json!(null), session_cookie, "").await;
    assert_eq!(s, 200);
    let context = cookie_header(&cookies);
    let token = v["data"]["token"].as_str().unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"Refresh"}),
            &format!("{session_cookie}; {context}"),
            token
        )
        .await
        .0,
        201
    );
    let a = imt_smart_teens_backend::modules::identity::domain::AdminPrincipal {
        id,
        name: "Synthetic".into(),
        email: "synthetic@example.invalid".into(),
        can_manage_schools: true,
        can_manage_content: true,
        school_ids: vec![],
    };
    assert!(a.require_schools().is_ok());
    assert!(a.require_school_scope(uuid::Uuid::new_v4()).is_ok());
}

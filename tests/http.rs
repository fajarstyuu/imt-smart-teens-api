mod common;
use common::*;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn school_lifecycle_enforces_revision_duplicates_and_archive(pool: PgPool) {
    let id = admin(&pool, true).await;
    let app = app(pool.clone()).await;
    let (c, t) = login(&app, id).await;
    let (s, v, _) = request(
        &app,
        "POST",
        "/api/v1/admin/schools",
        json!({"name":" École  STRAẞE "}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 201, "{v}");
    let sid = v["data"]["id"].as_str().unwrap();
    assert_eq!(v["data"]["revision"], 1);
    let (s, _, _) = request(
        &app,
        "POST",
        "/api/v1/admin/schools",
        json!({"name":"E\u{301}cole strasse"}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 409);
    for name in [" ".to_owned(), "x".repeat(301)] {
        assert_eq!(
            request(
                &app,
                "POST",
                "/api/v1/admin/schools",
                json!({"name":name}),
                &c,
                &t
            )
            .await
            .0,
            422
        );
    }
    let path = format!("/api/v1/admin/schools/{sid}");
    assert_eq!(
        request(
            &app,
            "PUT",
            &path,
            json!({"name":"Renamed","expected_revision":0}),
            &c,
            &t
        )
        .await
        .0,
        409
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            &path,
            json!({"name":"Renamed","expected_revision":1}),
            &c,
            &t
        )
        .await
        .0,
        200
    );
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("{path}?expected_revision=1"),
            json!(null),
            &c,
            &t
        )
        .await
        .0,
        409
    );
    let (s, v, _) = request(
        &app,
        "DELETE",
        &format!("{path}?expected_revision=2"),
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(v["data"]["revision"], 3);
    assert_eq!(
        request(
            &app,
            "DELETE",
            &format!("{path}?expected_revision=3"),
            json!(null),
            &c,
            &t
        )
        .await
        .1["data"]["revision"],
        3
    );
    assert_eq!(
        request(&app, "GET", "/api/v1/schools", json!(null), "", "")
            .await
            .1["meta"]["total"],
        0
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/admin/schools?status=archived",
            json!(null),
            &c,
            ""
        )
        .await
        .1["meta"]["total"],
        1
    );
    assert_eq!(
        request(
            &app,
            "PUT",
            &path,
            json!({"name":"Again","expected_revision":3}),
            &c,
            &t
        )
        .await
        .0,
        409
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"renamed"}),
            &c,
            &t
        )
        .await
        .0,
        409
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM platform_audit_events WHERE resource_type='school'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        3
    );
}
#[sqlx::test]
async fn sessions_csrf_permissions_expiry_and_logout(pool: PgPool) {
    let id = admin(&pool, true).await;
    let limited = admin(&pool, false).await;
    let app = app(pool.clone()).await;
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/me", json!(null), "", "")
            .await
            .0,
        401
    );
    let (c, t) = csrf(&app).await;
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/sessions",
            json!({"email":format!("{id}@example.invalid"),"password":"wrong"}),
            &c,
            &t
        )
        .await
        .0,
        401
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/sessions",
            json!({"email":"bad","password":"x"}),
            &c,
            &t
        )
        .await
        .0,
        422
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/sessions",
            json!({"email":"bad","password":"x"}),
            &c,
            ""
        )
        .await
        .0,
        403
    );
    let (c, t) = login(&app, id).await;
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
            json!({"name":"A"}),
            &c,
            "wrong"
        )
        .await
        .0,
        403
    );
    let (other_c, other_t) = csrf(&app).await;
    // A separately valid double-submit context cannot mutate this session.
    let session_cookie = c
        .split(';')
        .find(|v| v.trim().starts_with("imt_admin_session="))
        .unwrap();
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"A"}),
            &format!("{session_cookie}; {other_c}"),
            &other_t
        )
        .await
        .0,
        403
    );
    let (lc, lt) = login(&app, limited).await;
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/schools", json!(null), &lc, "")
            .await
            .0,
        403
    );
    assert_eq!(
        request(
            &app,
            "POST",
            "/api/v1/admin/schools",
            json!({"name":"A"}),
            &lc,
            &lt
        )
        .await
        .0,
        403
    );
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/api/v1/admin/schools/{}", uuid::Uuid::new_v4()),
            json!(null),
            &c,
            ""
        )
        .await
        .0,
        404
    );
    assert_eq!(
        request(
            &app,
            "DELETE",
            "/api/v1/admin/sessions/current",
            json!(null),
            &c,
            &t
        )
        .await
        .0,
        204
    );
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/me", json!(null), &c, "")
            .await
            .0,
        401
    );
    assert_eq!(
        request(
            &app,
            "DELETE",
            "/api/v1/admin/sessions/current",
            json!(null),
            &c,
            &t
        )
        .await
        .0,
        204
    );
    let (ec, _) = login(&app, id).await;
    sqlx::query(
        "UPDATE identity_sessions SET idle_expires_at=now()-interval '1 second' WHERE admin_id=$1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/me", json!(null), &ec, "")
            .await
            .0,
        401
    );
}
#[sqlx::test]
async fn health_errors_and_pagination_are_consistent(pool: PgPool) {
    let app = app(pool.clone()).await;
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
    assert_eq!(
        request(&app, "GET", "/api/v1/config", json!(null), "", "")
            .await
            .0,
        200
    );
    assert_eq!(
        request(&app, "GET", "/missing", json!(null), "", "")
            .await
            .0,
        404
    );
    assert_eq!(
        request(&app, "POST", "/health/live", json!(null), "", "")
            .await
            .0,
        405
    );
    assert_eq!(
        request(
            &app,
            "GET",
            "/api/v1/schools?limit=101",
            json!(null),
            "",
            ""
        )
        .await
        .0,
        422
    );
    pool.close().await;
    assert_eq!(
        request(&app, "GET", "/health/ready", json!(null), "", "")
            .await
            .0,
        503
    );
    assert_eq!(
        request(&app, "GET", "/health/live", json!(null), "", "")
            .await
            .0,
        200
    );
}

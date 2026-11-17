//! Global-admin regressions: removing any global capability or retaining a grant join must fail.
#[path = "fixtures/cms.rs"]
mod cms;
mod common;
#[path = "fixtures/http_student.rs"]
mod student;
use common::*;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn active_admin_is_global_without_legacy_grants(pool: PgPool) {
    cms::initialize(&pool).await;
    let (app, c, t, _) = cms::editor(&pool).await;
    let content = cms::publish(&app, &c, &t).await;
    let id = admin(&pool, false).await;
    let (ac, at) = login(&app, id).await;
    for path in [
        "/api/v1/admin/schools",
        "/api/v1/admin/lessons",
        "/api/v1/admin/question-banks",
    ] {
        let (status, body, _) = request(&app, "GET", path, json!(null), &ac, &at).await;
        assert_eq!(status, 200, "global admin {path}: {body}");
    }
    let (_, me, _) = request(&app, "GET", "/api/v1/admin/me", json!(null), &ac, &at).await;
    assert_eq!(me["data"]["admin"]["can_manage_schools"], true);
    assert_eq!(me["data"]["admin"]["can_manage_content"], true);
    let a = cms::school(&pool, "Global A").await;
    let (_, _, one) = student::enroll(&app, a).await;
    // New school is created AFTER login; no account grant or re-login is allowed.
    let b = cms::school(&pool, "Global B").await;
    let (sc, st, two) = student::enroll(&app, b).await;
    let attempt = student::attempt(&app, &sc, &st, "pre_test").await;
    let (status, all, _) = request(
        &app,
        "GET",
        "/api/v1/admin/students?limit=1",
        json!(null),
        &ac,
        &at,
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(all["meta"]["total"], 2);
    assert_eq!(all["data"].as_array().unwrap().len(), 1);
    for cohort in [&one, &two] {
        let url = format!(
            "/api/v1/admin/students/{}/results?enrollment_id={}",
            cohort["student"]["id"].as_str().unwrap(),
            cohort["enrollment"]["id"].as_str().unwrap()
        );
        assert_eq!(
            request(&app, "GET", &url, json!(null), &ac, &at).await.0,
            200
        );
    }
    let (_, filtered, _) = request(
        &app,
        "GET",
        &format!("/api/v1/admin/students?school_id={b}&offset=9"),
        json!(null),
        &ac,
        &at,
    )
    .await;
    assert_eq!(filtered["meta"]["total"], 1);
    assert_eq!(filtered["data"], json!([]));
    let url = format!(
        "/api/v1/admin/attempts/{}/answers",
        attempt["id"].as_str().unwrap()
    );
    assert_eq!(
        request(&app, "GET", &url, json!(null), &ac, &at).await.0,
        200
    );
    let question = &content.questions[0];
    let (_, detail, _) = request(
        &app,
        "GET",
        &format!(
            "/api/v1/admin/question-banks/{}/questions/{}",
            question["bank_id"].as_str().unwrap(),
            question["id"].as_str().unwrap()
        ),
        json!(null),
        &ac,
        &at,
    )
    .await;
    let version = detail["data"]["versions"][0]["id"].as_str().unwrap();
    let (_, answers, _) = request(
        &app,
        "GET",
        &format!("/api/v1/admin/reports/question-answers?question_version_id={version}"),
        json!(null),
        &ac,
        &at,
    )
    .await;
    assert_eq!(
        answers["meta"]["total"], 4,
        "pre/post pinned rows across both schools"
    );
    sqlx::query("UPDATE identity_schools SET archived_at=now() WHERE id=$1")
        .bind(b)
        .execute(&pool)
        .await
        .unwrap();
    let (_, archived, _) = request(
        &app,
        "GET",
        &format!("/api/v1/admin/students?school_id={b}"),
        json!(null),
        &ac,
        &at,
    )
    .await;
    assert_eq!(archived["meta"]["total"], 1);
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/students", json!(null), &sc, &st)
            .await
            .0,
        401
    );
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/students", json!(null), "", "")
            .await
            .0,
        401
    );
    sqlx::query("UPDATE identity_admins SET active=false WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(&app, "GET", "/api/v1/admin/students", json!(null), &ac, &at)
            .await
            .0,
        401
    );
}

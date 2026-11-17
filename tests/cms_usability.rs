//! Real PostgreSQL acceptance for the reviewed, all-or-nothing CMS release command.
#[path = "fixtures/cms.rs"]
mod cms;
mod common;
#[path = "fixtures/http_student.rs"]
mod student;
use common::*;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;
const RELEASE: &str = "/api/v1/admin/learning-path/release";
async fn setup(pool: &PgPool) -> (axum::Router, String, String, Value) {
    cms::initialize(pool).await;
    let (app, c, t, _) = cms::editor(pool).await;
    let (s,l,_)=request(&app,"POST","/api/v1/admin/lessons",json!({"title":"Reviewed lesson","description":"","sections":[{"title":"Reviewed lesson","body":"Reviewed body","required":true}],"position":1}),&c,&t).await;
    assert_eq!(s, 201, "{l}");
    let (_, bs, _) = request(
        &app,
        "GET",
        "/api/v1/admin/question-banks",
        json!(null),
        &c,
        &t,
    )
    .await;
    for b in bs["data"].as_array().unwrap() {
        let a = Uuid::new_v4();
        let z = Uuid::new_v4();
        let (s,q,_)=request(&app,"POST",&format!("/api/v1/admin/question-banks/{}/questions",b["id"].as_str().unwrap()),json!({"text":"Reviewed question","type":"single","options":[{"id":a,"text":"One"},{"id":z,"text":"Two"}],"correct_option_ids":[a],"required":true,"position":1}),&c,&t).await;
        assert_eq!(s, 201, "{q}");
    }
    let (_, p, _) = request(
        &app,
        "GET",
        "/api/v1/admin/learning-path",
        json!(null),
        &c,
        &t,
    )
    .await;
    let (_, bs, _) = request(
        &app,
        "GET",
        "/api/v1/admin/question-banks",
        json!(null),
        &c,
        &t,
    )
    .await;
    let command = json!({"expected_revision":p["data"]["revision"],"lessons":[{"id":l["data"]["id"],"expected_revision":l["data"]["revision"]}],"banks":bs["data"].as_array().unwrap().iter().map(|b|json!({"id":b["id"],"expected_revision":b["revision"]})).collect::<Vec<_>>()});
    (app, c, t, command)
}
async fn versions(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT (SELECT count(*) FROM learning_path_versions)+(SELECT count(*) FROM learning_lesson_versions)+(SELECT count(*) FROM assessment_bank_versions)+(SELECT count(*) FROM assessment_question_versions)").fetch_one(pool).await.unwrap()
}
#[sqlx::test(migrations = "./migrations")]
async fn release_publishes_drafts_once_and_preserves_existing_enrollment(pool: PgPool) {
    let (app, c, t, cmd) = setup(&pool).await;
    let (s, p, _) = request(&app, "POST", RELEASE, cmd.clone(), &c, &t).await;
    assert_eq!(s, 200, "{p}");
    assert_eq!(versions(&pool).await, 6);
    let old = p["data"]["published"]["id"].clone();
    let school = cms::school(&pool, "Synthetic release school").await;
    let (_, _, profile) = student::enroll(&app, school).await;
    assert_eq!(profile["enrollment"]["path_version_id"], old);
    assert_eq!(request(&app, "POST", RELEASE, cmd, &c, &t).await.0, 409);
    assert_eq!(versions(&pool).await, 6);
}
#[sqlx::test(migrations = "./migrations")]
async fn stale_review_or_missing_membership_never_creates_any_versions(pool: PgPool) {
    let (app, c, t, cmd) = setup(&pool).await;
    for kind in ["path", "lesson", "bank", "missing", "duplicate"] {
        let mut stale = cmd.clone();
        match kind {
            "path" => stale["expected_revision"] = json!(99),
            "lesson" => stale["lessons"][0]["expected_revision"] = json!(99),
            "bank" => stale["banks"][1]["expected_revision"] = json!(99),
            "missing" => stale["lessons"] = json!([]),
            _ => stale["banks"][1] = stale["banks"][0].clone(),
        }
        let (s, v, _) = request(&app, "POST", RELEASE, stale, &c, &t).await;
        assert!(s == 409 || s == 422, "{kind}: {s} {v}");
        assert_eq!(versions(&pool).await, 0);
    }
}
#[sqlx::test(migrations = "./migrations")]
async fn invalid_bank_rolls_back_other_bank_and_lesson_publications(pool: PgPool) {
    let (app, c, t, mut cmd) = setup(&pool).await;
    let id = cmd["banks"][1]["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    sqlx::query("UPDATE assessment_questions SET state='archived' WHERE bank_id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    cmd["banks"][1]["expected_revision"] = json!(2);
    let (s, v, _) = request(&app, "POST", RELEASE, cmd, &c, &t).await;
    assert_eq!(s, 422, "{v}");
    assert_eq!(versions(&pool).await, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM platform_audit_events WHERE action LIKE '%.publish'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn concurrent_release_exactly_one_wins_and_csrf_admin_are_required(pool: PgPool) {
    let (app, c, t, cmd) = setup(&pool).await;
    assert_eq!(
        request(&app, "POST", RELEASE, cmd.clone(), "", "").await.0,
        403
    );
    assert_eq!(
        request(&app, "POST", RELEASE, cmd.clone(), &c, "").await.0,
        403
    );
    let (a, b) = tokio::join!(
        request(&app, "POST", RELEASE, cmd.clone(), &c, &t),
        request(&app, "POST", RELEASE, cmd, &c, &t)
    );
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    assert_eq!(versions(&pool).await, 6);
}
#[sqlx::test(migrations = "./migrations")]
async fn student_name_search_is_bounded_and_missing_scores_stay_null(pool: PgPool) {
    let (app, c, t, cmd) = setup(&pool).await;
    assert_eq!(request(&app, "POST", RELEASE, cmd, &c, &t).await.0, 200);
    let school = cms::school(&pool, "Synthetic search school").await;
    let (_, _, profile) = student::enroll(&app, school).await;
    let id = profile["student"]["id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    sqlx::query("UPDATE identity_students SET display_name='Literal % Needle' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let (s, p, _) = request(
        &app,
        "GET",
        "/api/v1/admin/students?q=needle&limit=1",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200, "{p}");
    assert_eq!(p["meta"]["total"], 1);
    assert!(p["data"][0]["pre_test_score"].is_null());
    assert!(p["data"][0]["post_test_score"].is_null());
    assert!(p["data"][0]["latest_activity_at"].is_string());
    let (s, p, _) = request(
        &app,
        "GET",
        "/api/v1/admin/students?q=absent",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(p["meta"]["total"], 0);
}

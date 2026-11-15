//! PostgreSQL locks and publication rollback are exercised against real transactions.
#[path = "fixtures/cms.rs"]
mod cms;
mod common;
use common::*;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;
#[sqlx::test(migrations = "./migrations")]
async fn concurrent_bank_publications_and_path_orders_have_one_revision_winner(pool: PgPool) {
    cms::initialize(&pool).await;
    let (app, c, t, _) = cms::editor(&pool).await;
    let content = cms::publish(&app, &c, &t).await;
    let bank = &content.banks[0];
    let path = format!(
        "/api/v1/admin/question-banks/{}/publication",
        bank["id"].as_str().unwrap()
    );
    let payload = json!({"expected_revision":bank["revision"]});
    let (a, b) = tokio::join!(
        request(&app, "POST", &path, payload.clone(), &c, &t),
        request(&app, "POST", &path, payload, &c, &t)
    );
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM assessment_bank_versions WHERE bank_id=$1")
            .bind(bank["id"].as_str().unwrap().parse::<Uuid>().unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
    let (s, l, _) = request(
        &app,
        "POST",
        "/api/v1/admin/lessons",
        json!({"title":"Second","description":"","sections":[],"position":2}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 201, "{l}");
    let ids = json!([content.lesson["id"], l["data"]["id"]]);
    let reverse = json!([l["data"]["id"], content.lesson["id"]]);
    let (a, b) = tokio::join!(
        request(
            &app,
            "PUT",
            "/api/v1/admin/learning-path/order",
            json!({"lesson_ids":ids,"expected_revision":4}),
            &c,
            &t
        ),
        request(
            &app,
            "PUT",
            "/api/v1/admin/learning-path/order",
            json!({"lesson_ids":reverse,"expected_revision":4}),
            &c,
            &t
        )
    );
    let mut statuses = [a.0, b.0];
    statuses.sort();
    assert_eq!(statuses, [200, 409]);
    let (_, p, _) = request(
        &app,
        "GET",
        "/api/v1/admin/learning-path",
        json!(null),
        &c,
        &t,
    )
    .await;
    assert_eq!(p["data"]["revision"], 5);
    let winner = if a.0 == 200 { a.1 } else { b.1 };
    assert_eq!(p["data"]["lesson_ids"], winner["data"]["lesson_ids"]);
}
#[sqlx::test(migrations = "./migrations")]
async fn publication_revalidates_persisted_drafts_order_and_rolls_back_path_audit(pool: PgPool) {
    cms::initialize(&pool).await;
    let (app, c, t, _) = cms::editor(&pool).await;
    let content = cms::publish(&app, &c, &t).await;
    let question = &content.questions[0];
    let id: Uuid = question["id"].as_str().unwrap().parse().unwrap();
    let bank = question["bank_id"].as_str().unwrap();
    // Simulate incompatible/corrupt stored editor data: publication must not trust a prior save.
    sqlx::query("UPDATE assessment_questions SET draft_payload=jsonb_set(draft_payload,'{correct_option_ids}','[]'),revision=revision+1 WHERE id=$1").bind(id).execute(&pool).await.unwrap();
    let (s, v, _) = request(
        &app,
        "POST",
        &format!("/api/v1/admin/question-banks/{bank}/publication"),
        json!({"expected_revision":4}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 422, "{v}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM assessment_bank_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    sqlx::raw_sql("CREATE FUNCTION fail_path_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action='path.publish' THEN RAISE EXCEPTION 'synthetic'; END IF; RETURN NEW; END $$; CREATE TRIGGER fail_path_audit BEFORE INSERT ON platform_audit_events FOR EACH ROW EXECUTE FUNCTION fail_path_audit()").execute(&pool).await.unwrap();
    // Dirty drafts do not affect previously published data; path can validate old immutable keys.
    let (s, v, _) = request(
        &app,
        "POST",
        "/api/v1/admin/learning-path/publication",
        json!({"expected_revision":3}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 500, "{v}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM learning_path_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT revision FROM learning_path_drafts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
}
#[allow(dead_code)]
#[path = "fixtures/student.rs"]
mod fixture;
#[sqlx::test(migrations = "./migrations")]
async fn path_revalidates_exact_aggregate_score_budget_before_writes(pool: PgPool) {
    let (_, lessons) =
        fixture::weighted_content(&pool, [["50000000", "50000000"], ["1", "1"]]).await;
    cms::initialize(&pool).await;
    sqlx::query("UPDATE learning_path_drafts SET lesson_ids=$1")
        .bind(&lessons)
        .execute(&pool)
        .await
        .unwrap();
    let (app, c, t, _) = cms::editor(&pool).await;
    let (s, v, _) = request(
        &app,
        "POST",
        "/api/v1/admin/learning-path/publication",
        json!({"expected_revision":1}),
        &c,
        &t,
    )
    .await;
    assert_eq!(s, 409, "{v}");
    assert_eq!(v["error"]["code"], "CONTENT_NOT_AVAILABLE");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM learning_path_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT revision FROM learning_path_drafts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}

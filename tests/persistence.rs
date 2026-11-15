//! Real PostgreSQL constraints, transactional history, and concurrent catalog writes.
mod common;
use common::*;
use serde_json::json;
use sqlx::PgPool;
async fn fixture(pool: &PgPool) {
    sqlx::raw_sql(include_str!("fixtures/relational.sql"))
        .execute(pool)
        .await
        .unwrap();
}
fn id(n: u128) -> uuid::Uuid {
    uuid::Uuid::parse_str(&format!("00000000-0000-0000-0000-{n:012}")).unwrap()
}
async fn rejected(pool: &PgPool, sql: &str, code: &str) {
    let error = sqlx::raw_sql(sql).execute(pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some(code)
    );
}
#[sqlx::test]
async fn composite_foreign_keys_reject_cross_owner_and_cross_question_rows(pool: PgPool) {
    fixture(&pool).await;
    rejected(
        &pool,
        "UPDATE assessment_attempts SET enrollment_id='00000000-0000-0000-0000-000000000019'",
        "23503",
    )
    .await;
    rejected(
        &pool,
        "UPDATE assessment_answers SET snapshot_question_id='00000000-0000-0000-0000-000000000023'",
        "23503",
    )
    .await;
    rejected(&pool,"UPDATE assessment_answer_options SET question_version_id='00000000-0000-0000-0000-000000000009',option_id='00000000-0000-0000-0000-000000000011'", "23503").await;
    rejected(&pool,"INSERT INTO identity_sessions(id,token_hash,role,student_id,enrollment_id,csrf_binding_hash,idle_expires_at,absolute_expires_at) VALUES('00000000-0000-0000-0000-000000000026',decode(repeat('01',32),'hex'),'student','00000000-0000-0000-0000-000000000003','00000000-0000-0000-0000-000000000018',decode(repeat('02',32),'hex'),now()+interval '1 hour',now()+interval '2 hours')", "23503").await;
    rejected(&pool,"INSERT INTO learning_section_visits(enrollment_id,lesson_version_id,section_id) VALUES('00000000-0000-0000-0000-000000000019','00000000-0000-0000-0000-000000000015','00000000-0000-0000-0000-000000000016')", "23503").await;
    rejected(&pool,"INSERT INTO assessment_attempts(id,enrollment_id,snapshot_id,assessment_type,attempt_number,status) SELECT '00000000-0000-0000-0000-000000000027',enrollment_id,snapshot_id,assessment_type,2,status FROM assessment_attempts", "23505").await;
    rejected(&pool, "DELETE FROM identity_schools", "23503").await;
    rejected(&pool, "DELETE FROM assessment_question_versions", "23503").await;
    // Fixed-scale NUMERIC rejects infinities at conversion, before CHECK evaluation.
    for (value, code) in [
        ("0", "23514"),
        ("-1", "23514"),
        ("'NaN'", "23514"),
        ("'Infinity'", "22003"),
        ("'-Infinity'", "22003"),
    ] {
        rejected(
            &pool,
            &format!("UPDATE anthropometry_measurements SET weight_kg={value}"),
            code,
        )
        .await;
    }
}
#[sqlx::test]
async fn rename_archive_preserve_history_and_existing_student_session(pool: PgPool) {
    fixture(&pool).await;
    let admin_id = admin(&pool, true).await;
    let app = app(pool.clone()).await;
    let (c, t) = login(&app, admin_id).await;
    let path = format!("/api/v1/admin/schools/{}", id(1));
    assert_eq!(
        request(
            &app,
            "PUT",
            &path,
            json!({"name":"New label","expected_revision":1}),
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
            &format!("{path}?expected_revision=2"),
            json!(null),
            &c,
            &t
        )
        .await
        .0,
        200
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT school_name_at_enrollment FROM learning_enrollments WHERE id=$1"
        )
        .bind(id(18))
        .fetch_one(&pool)
        .await
        .unwrap(),
        "Synthetic school"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM assessment_answer_options")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM anthropometry_measurements")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}
#[sqlx::test]
async fn concurrent_school_edits_have_one_winner_and_audit_is_atomic(pool: PgPool) {
    let admin_id = admin(&pool, true).await;
    let app = app(pool.clone()).await;
    let (c, t) = login(&app, admin_id).await;
    let (_, v, _) = request(
        &app,
        "POST",
        "/api/v1/admin/schools",
        json!({"name":"Race"}),
        &c,
        &t,
    )
    .await;
    let path = format!(
        "/api/v1/admin/schools/{}",
        v["data"]["id"].as_str().unwrap()
    );
    let (a, b) = tokio::join!(
        request(
            &app,
            "PUT",
            &path,
            json!({"name":"First","expected_revision":1}),
            &c,
            &t
        ),
        request(
            &app,
            "PUT",
            &path,
            json!({"name":"Second","expected_revision":1}),
            &c,
            &t
        )
    );
    let mut codes = [a.0, b.0];
    codes.sort();
    assert_eq!(codes, [200, 409]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM platform_audit_events WHERE action='school.renamed'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

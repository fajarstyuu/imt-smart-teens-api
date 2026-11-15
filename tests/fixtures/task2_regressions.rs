//! Fix-round regressions use real immutable publications and transaction barriers.
use super::*;
use imt_smart_teens_backend::{
    bootstrap::assemble,
    modules::learning::{
        application::{
            StudentService,
            ports::{StudentTransaction, StudentUnitOfWork},
        },
        infrastructure::PgStudentUnitOfWork,
    },
    shared::error::AppResult,
    transport::http::build_router,
};
use std::sync::Arc;

async fn budget_rejection(pool: PgPool, weights: [[&str; 2]; 2]) {
    let (school, _) = fixture::weighted_content(&pool, weights).await;
    let app = app(pool.clone()).await;
    let (c, t) = csrf(&app).await;
    let input = json!({"name":"Budget student","school_id":school,"form_date":"2026-11-15"});
    let (status, body, cookies) = call(
        &app,
        "POST",
        "/api/v1/student/sessions",
        input.clone(),
        &c,
        &t,
        "budget",
    )
    .await;
    assert_eq!(
        status, 409,
        "registration must reject unrepresentable content"
    );
    assert_eq!(body["error"]["code"], "CONTENT_NOT_AVAILABLE");
    assert!(cookies.is_empty());
    for table in [
        "identity_students",
        "learning_enrollments",
        "identity_sessions",
        "assessment_snapshots",
        "assessment_attempts",
        "platform_idempotency",
        "platform_audit_events",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT count(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0,
            "partial effect: {table}"
        );
    }
    // Publish a genuinely new valid path; do not mutate sealed question versions.
    // The fixture's future paired version is sufficient when only paired is invalid.
    if weights[1] == ["1", "1"] {
        fixture::publish_future(&pool).await;
        let (status, _body, _) = call(
            &app,
            "POST",
            "/api/v1/student/sessions",
            input,
            &c,
            &t,
            "budget",
        )
        .await;
        assert_eq!(status, 201, "failed key was not retryable");
    }
}
#[sqlx::test(migrations = "./migrations")]
async fn paired_score_budget_rejects_atomically(pool: PgPool) {
    budget_rejection(pool, [["99999999.9999", "99999999.9999"], ["1", "1"]]).await;
}
#[sqlx::test(migrations = "./migrations")]
async fn quiz_score_budget_rejects_atomically(pool: PgPool) {
    budget_rejection(pool, [["1", "1"], ["99999999.9999", "99999999.9999"]]).await;
}
#[sqlx::test(migrations = "./migrations")]
async fn one_unit_above_score_budget_rejects_atomically(pool: PgPool) {
    budget_rejection(pool, [["99999999.9999", "0.0001"], ["1", "1"]]).await;
}
#[sqlx::test(migrations = "./migrations")]
async fn exact_score_budget_submits_without_rounding_or_overflow(pool: PgPool) {
    let (school, _) =
        fixture::weighted_content(&pool, [["99999999.9998", "0.0001"], ["1", "1"]]).await;
    let app = app(pool.clone()).await;
    let (c, t, _) = enroll(&app, school).await;
    let a = attempt(&app, &c, &t, "pre_test").await;
    answer_all(&app, &c, &t, &a, true).await;
    submit(&app, &c, &t, &a, "boundary").await;
    let values:(String,String,String)=sqlx::query_as("SELECT earned_points::text,max_points::text,score_percent::text FROM assessment_attempts WHERE id=$1").bind(a["id"].as_str().unwrap().parse::<Uuid>().unwrap()).fetch_one(&pool).await.unwrap();
    assert_eq!(
        values,
        (
            "99999999.9999".into(),
            "99999999.9999".into(),
            "100.00".into()
        )
    );
}

/// Pause only the first transaction after real BEGIN but before returning it to the workflow.
struct BeginBarrier {
    real: PgStudentUnitOfWork,
    gate: tokio::sync::Mutex<
        Option<(
            tokio::sync::oneshot::Sender<()>,
            tokio::sync::oneshot::Receiver<()>,
        )>,
    >,
}
#[async_trait::async_trait]
impl StudentUnitOfWork for BeginBarrier {
    async fn begin(&self) -> AppResult<Box<dyn StudentTransaction>> {
        let tx = self.real.begin().await?;
        let gate = self.gate.lock().await.take();
        if let Some((begun, release)) = gate {
            begun.send(()).unwrap();
            release.await.unwrap();
        }
        Ok(tx)
    }
}
#[sqlx::test(migrations = "./migrations")]
async fn earlier_completion_transaction_observes_later_opening(pool: PgPool) {
    let (school, lessons) = content(&pool).await;
    let app = app(pool.clone()).await;
    let (c, t, _) = enroll(&app, school).await;
    let pre = attempt(&app, &c, &t, "pre_test").await;
    answer_all(&app, &c, &t, &pre, true).await;
    submit(&app, &c, &t, &pre, "pre").await;
    let path = format!("/api/v1/student/lessons/{}", lessons[0]);
    let (_, view, _) = call(&app, "GET", &path, json!(null), &c, &t, "").await;
    let section = view["data"]["sections"][0]["id"].clone();
    let (begun_tx, begun_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let mut state = assemble(pool.clone(), settings()).await.unwrap();
    state.students = StudentService::new(
        Arc::new(BeginBarrier {
            real: PgStudentUnitOfWork::new(pool.clone()),
            gate: tokio::sync::Mutex::new(Some((begun_tx, release_rx))),
        }),
        state.sessions.clone(),
    );
    let completion_app = build_router(state);
    let cp = format!("{path}/completion");
    let cc = c.clone();
    let ct = t.clone();
    let completion =
        tokio::spawn(
            async move { call(&completion_app, "POST", &cp, json!({}), &cc, &ct, "").await },
        );
    begun_rx.await.unwrap(); // Real completion BEGIN precedes opening BEGIN.
    let (s, opened, _) = call(
        &app,
        "POST",
        &format!("{path}/opened"),
        json!({"section_id":section}),
        &c,
        &t,
        "",
    )
    .await;
    assert_eq!(s, 200, "{opened}"); // Opening and its visit have committed before completion can lock.
    release_tx.send(()).unwrap();
    let (s, completed, _) = completion.await.unwrap();
    assert_eq!(s, 200, "earlier transaction completion failed: {completed}");
    let opened_at =
        chrono::DateTime::parse_from_rfc3339(opened["data"]["opened_at"].as_str().unwrap())
            .unwrap();
    let completed_at =
        chrono::DateTime::parse_from_rfc3339(completed["data"]["completed_at"].as_str().unwrap())
            .unwrap();
    assert!(completed_at >= opened_at);
    let (s, repeated, _) = call(
        &app,
        "POST",
        &format!("{path}/completion"),
        json!({}),
        &c,
        &t,
        "",
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(
        repeated["data"]["completed_at"],
        completed["data"]["completed_at"]
    );
}

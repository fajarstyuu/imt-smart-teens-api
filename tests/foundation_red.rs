//! These boundary tests start against an empty router/schema, then drive composition.
use axum::{body::Body, http::Request};
mod common;
use tower::ServiceExt;

#[tokio::test]
async fn live_route_is_available_without_database() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .unwrap();
    let app = common::app(pool).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        200,
        "the composition root must register liveness"
    );
}

#[sqlx::test(migrations = false)]
async fn empty_database_gets_full_relational_schema(pool: sqlx::PgPool) {
    imt_smart_teens_backend::platform::database::MIGRATOR
        .run(&pool)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND table_name <> '_sqlx_migrations'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        count, 30,
        "all 30 business tables must migrate from empty PostgreSQL"
    );
}

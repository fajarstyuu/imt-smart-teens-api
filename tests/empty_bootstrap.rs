//! Production-like synthetic checks for the explicit structural-root bootstrap command.
mod common;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn empty_bootstrap_is_production_safe_idempotent_and_preserves_admin(pool: PgPool) {
    use sqlx::ConnectOptions;

    let admin_id = Uuid::new_v4();
    let original_hash = "$argon2id$v=19$m=19456,t=2,p=1$synthetic$synthetic";
    sqlx::query("INSERT INTO identity_admins(id,email,display_name,password_hash) VALUES($1,'admin@example.invalid','Existing admin',$2)")
        .bind(admin_id)
        .bind(original_hash)
        .execute(&pool)
        .await
        .unwrap();
    let url = pool.connect_options().to_url_lossy().to_string();
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_imt-smart-teens-backend"))
            .arg("init-empty-content")
            .env("APP_ENV", "production")
            .env("DATABASE_URL", &url)
            .env(
                "CSRF_SIGNING_KEY",
                "synthetic-test-key-not-for-deployment-000000000000",
            )
            .env("SESSION_COOKIE_SECURE", "true")
            .env("CORS_ALLOWED_ORIGINS", "https://example.invalid")
            .output()
            .unwrap()
    };

    assert!(
        run().status.success(),
        "explicit empty bootstrap command failed"
    );
    let roots: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM assessment_banks),(SELECT count(*) FROM learning_path_drafts),(SELECT count(*) FROM learning_lessons)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(roots, (2, 1, 0));
    let path_before: (Uuid, i64, Vec<Uuid>) =
        sqlx::query_as("SELECT id,revision,lesson_ids FROM learning_path_drafts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(run().status.success());
    let path_after: (Uuid, i64, Vec<Uuid>) =
        sqlx::query_as("SELECT id,revision,lesson_ids FROM learning_path_drafts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(path_after, path_before);

    let hash: String = sqlx::query_scalar("SELECT password_hash FROM identity_admins WHERE id=$1")
        .bind(admin_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(hash, original_hash);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM learning_lesson_versions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

//! Real private operator CLI: idempotence/concurrency must never rotate an existing credential.
use sqlx::{ConnectOptions, PgPool};
use std::{
    io::Write,
    process::{Child, Command, Stdio},
};
fn seed(pool: &PgPool, email: &str, password: &str) -> Child {
    let mut child = Command::new(env!("CARGO_BIN_EXE_imt-smart-teens-backend"))
        .arg("seed-admin")
        .env(
            "DATABASE_URL",
            pool.connect_options().to_url_lossy().to_string(),
        )
        .env("APP_ENV", "production")
        .env("SESSION_COOKIE_SECURE", "true")
        .env(
            "CSRF_SIGNING_KEY",
            "synthetic-seed-key-not-a-production-secret-000000000",
        )
        .env("CORS_ALLOWED_ORIGINS", "https://example.invalid")
        .env("ADMIN_EMAIL", email)
        .env("ADMIN_NAME", "Synthetic operator")
        .env_remove("ADMIN_PASSWORD")
        .env("RUST_LOG", "info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(password.as_bytes())
        .unwrap();
    child
}
async fn snapshot(pool: &PgPool) -> (uuid::Uuid, String, String, bool) {
    sqlx::query_as("SELECT id,password_hash,display_name,active FROM identity_admins WHERE email='seed-synthetic@example.invalid'").fetch_one(pool).await.unwrap()
}
#[sqlx::test(migrations = "./migrations")]
async fn seed_admin_is_idempotent_and_private(pool: PgPool) {
    let password = "synthetic-only-seed-passphrase-1234";
    let first = seed(&pool, " Seed-Synthetic@example.invalid ", password)
        .wait_with_output()
        .unwrap();
    assert!(first.status.success(), "seed command must create an admin");
    let before = snapshot(&pool).await;
    assert!(before.1.starts_with("$argon2id$"));
    let second = seed(
        &pool,
        "seed-synthetic@example.invalid",
        "different-synthetic-password-5678",
    )
    .wait_with_output()
    .unwrap();
    assert!(second.status.success());
    assert_eq!(before, snapshot(&pool).await);
    assert!(String::from_utf8_lossy(&first.stdout).contains("created"));
    assert!(String::from_utf8_lossy(&second.stdout).contains("unchanged"));
    for out in [&first, &second] {
        for bytes in [&out.stdout, &out.stderr] {
            let text = String::from_utf8_lossy(bytes);
            assert!(!text.contains(password));
            assert!(!text.contains("different-synthetic-password-5678"));
            assert!(!text.contains("postgres://"));
        }
    }
    sqlx::query("UPDATE identity_admins SET active=false WHERE id=$1")
        .bind(before.0)
        .execute(&pool)
        .await
        .unwrap();
    let inactive = snapshot(&pool).await;
    let existing = seed(&pool, "seed-synthetic@example.invalid", password)
        .wait_with_output()
        .unwrap();
    assert!(existing.status.success());
    assert_eq!(inactive, snapshot(&pool).await);
}
#[sqlx::test(migrations = "./migrations")]
async fn seed_admin_concurrent_normalized_email_preserves_identity(pool: PgPool) {
    let a = seed(
        &pool,
        "SEED-SYNTHETIC@example.invalid",
        "synthetic-concurrent-password-A",
    );
    let b = seed(
        &pool,
        "seed-synthetic@example.invalid",
        "synthetic-concurrent-password-B",
    );
    let a = a.wait_with_output().unwrap();
    let b = b.wait_with_output().unwrap();
    assert!(a.status.success() && b.status.success());
    let texts = [
        String::from_utf8_lossy(&a.stdout),
        String::from_utf8_lossy(&b.stdout),
    ];
    assert_eq!(texts.iter().filter(|s| s.contains("created")).count(), 1);
    assert_eq!(texts.iter().filter(|s| s.contains("unchanged")).count(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity_admins")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
}
#[sqlx::test(migrations = "./migrations")]
async fn seed_admin_rejects_invalid_password_without_account(pool: PgPool) {
    assert!(
        !seed(&pool, "seed-synthetic@example.invalid", "short")
            .wait_with_output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM identity_admins")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

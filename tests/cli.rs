//! Exercise real CLI migration/provisioning and an owned loopback server process.
//! Child credentials are passed privately through environment, never printed.
use sqlx::{ConnectOptions, PgPool};
use std::{
    process::{Child, Command, Stdio},
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn command(url: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_imt-smart-teens-backend"));
    command
        .env("DATABASE_URL", url)
        .env("MIGRATION_DATABASE_URL", url)
        .env("APP_ENV", "test")
        .env(
            "CSRF_SIGNING_KEY",
            "synthetic-cli-test-key-not-for-deployment-000000",
        )
        .env("CORS_ALLOWED_ORIGINS", "http://localhost:5173")
        .env("HTTP_HOST", "127.0.0.1")
        .env("RUST_LOG", "warn")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}
#[sqlx::test(migrations = false)]
async fn private_cli_migrates_provisions_and_serves_with_graceful_sigterm(pool: PgPool) {
    let url = pool.connect_options().to_url_lossy().to_string();
    // Every command targets this SQLx-created database, never the application DB.
    assert!(
        command(&url).arg("migrate").status().unwrap().success(),
        "explicit migration CLI failed"
    );
    assert!(
        command(&url).arg("migrate").status().unwrap().success(),
        "migration replay must be safe"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
    let mut provision = command(&url);
    provision
        .arg("provision-admin")
        .env("ADMIN_EMAIL", "cli-synthetic@example.invalid")
        .env("ADMIN_NAME", "Synthetic CLI admin")
        .env("ADMIN_PASSWORD", "synthetic-private-cli-password-234")
        .env("ADMIN_CAN_MANAGE_SCHOOLS", "true")
        .env("ADMIN_CAN_MANAGE_CONTENT", "false");
    assert!(
        provision.status().unwrap().success(),
        "private provisioning CLI failed"
    );
    let(hash,schools,content):(String,bool,bool)=sqlx::query_as("SELECT password_hash,can_manage_schools,can_manage_content FROM identity_admins WHERE email='cli-synthetic@example.invalid'").fetch_one(&pool).await.unwrap();
    assert!(hash.starts_with("$argon2id$"));
    assert!(schools);
    assert!(!content);
    assert!(
        !command(&url)
            .arg("provision-admin")
            .arg("plaintext-password-must-not-be-an-argument")
            .status()
            .unwrap()
            .success()
    );
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let mut server = OwnedChild(
        command(&url)
            .arg("serve")
            .env("HTTP_PORT", address.port().to_string())
            .spawn()
            .unwrap(),
    );
    let mut ready = false;
    for _ in 0..100 {
        if let Ok(mut socket) = tokio::net::TcpStream::connect(address).await {
            socket
                .write_all(
                    b"GET /health/ready HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
            let mut bytes = Vec::new();
            tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut bytes))
                .await
                .unwrap()
                .unwrap();
            let response = String::from_utf8(bytes).unwrap();
            assert!(response.starts_with("HTTP/1.1 200"));
            assert!(response.contains("ready"));
            ready = true;
            break;
        }
        assert!(
            server.0.try_wait().unwrap().is_none(),
            "server exited before readiness"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(ready, "loopback server did not become ready");
    assert!(
        Command::new("kill")
            .arg("-TERM")
            .arg(server.0.id().to_string())
            .status()
            .unwrap()
            .success()
    );
    let mut stopped = false;
    for _ in 0..100 {
        if let Some(status) = server.0.try_wait().unwrap() {
            assert!(status.success(), "graceful server exit failed");
            stopped = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(stopped, "owned server exceeded shutdown test deadline");
}

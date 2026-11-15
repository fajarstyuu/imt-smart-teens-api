//! Exercise real CLI migration/provisioning and an owned loopback server process.
//! Child credentials are passed privately through environment, never printed.
mod common;
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

/// Bound every network probe separately from the child-exit deadline. The guard
/// kills/reaps only this test's child even when an assertion fails on old code.
async fn exchange(address: std::net::SocketAddr, request: &str) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(2), async {
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket.write_all(request.as_bytes()).await.unwrap();
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes).await.unwrap();
        bytes
    })
    .await
    .expect("owned HTTP exchange exceeded deadline")
}

#[cfg(target_os = "linux")]
#[sqlx::test]
async fn sigterm_deadline_bounds_active_transaction_and_pool_cleanup(pool: PgPool) {
    use common::*;
    use serde_json::json;
    let admin_id = admin(&pool, true).await;
    let app = app(pool.clone()).await;
    let (cookie, token) = login(&app, admin_id).await;
    let (status, value, _) = request(
        &app,
        "POST",
        "/api/v1/admin/schools",
        json!({"name":"Before shutdown"}),
        &cookie,
        &token,
    )
    .await;
    assert_eq!(status, 201);
    let school_id = uuid::Uuid::parse_str(value["data"]["id"].as_str().unwrap()).unwrap();
    // Installed only inside this SQLx-owned database: sleep after the school
    // UPDATE but before the audit INSERT/commit, proving real rollback on exit.
    sqlx::raw_sql("CREATE FUNCTION test_block_audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.action = 'school.renamed' THEN PERFORM pg_sleep(4); END IF; RETURN NEW; END $$; CREATE TRIGGER test_block_audit BEFORE INSERT ON platform_audit_events FOR EACH ROW EXECUTE FUNCTION test_block_audit();").execute(&pool).await.unwrap();
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = probe.local_addr().unwrap();
    drop(probe);
    let url = pool.connect_options().to_url_lossy().to_string();
    let mut server = OwnedChild(
        command(&url)
            .arg("serve")
            .env("HTTP_PORT", address.port().to_string())
            .env(
                "CSRF_SIGNING_KEY",
                "synthetic-test-key-not-for-deployment-000000000000",
            )
            .env("SHUTDOWN_TIMEOUT_SECS", "1")
            .env("DATABASE_MAX_CONNECTIONS", "1")
            .env("REQUEST_TIMEOUT_SECS", "30")
            .env("DATABASE_STATEMENT_TIMEOUT_SECS", "30")
            .spawn()
            .unwrap(),
    );
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            assert!(
                server.0.try_wait().unwrap().is_none(),
                "server exited before readiness"
            );
            if tokio::net::TcpStream::connect(address).await.is_ok() {
                let response = exchange(
                    address,
                    "GET /health/ready HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                )
                .await;
                if response.starts_with(b"HTTP/1.1 200") {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("owned server never became ready");
    let body = json!({"name":"Must never commit","expected_revision":1}).to_string();
    let mut active = tokio::net::TcpStream::connect(address).await.unwrap();
    let edit = format!(
        "PUT /api/v1/admin/schools/{school_id} HTTP/1.1\r\nHost: localhost\r\nOrigin: http://localhost:5173\r\nCookie: {cookie}\r\nX-CSRF-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    active.write_all(edit.as_bytes()).await.unwrap();
    // Database evidence, not a sleep, proves the child holds an active transaction.
    let backend_pid = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let pid: Option<i32> = sqlx::query_scalar(
                "SELECT pid FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND state='active' AND wait_event='PgSleep'"
            ).fetch_optional(&pool).await.unwrap();
            if let Some(pid) = pid { break pid; }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }).await.expect("HTTP transaction never reached blocking audit trigger");
    let started = std::time::Instant::now();
    assert!(
        Command::new("kill")
            .arg("-TERM")
            .arg(server.0.id().to_string())
            .status()
            .unwrap()
            .success()
    );
    let status = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let Some(status) = server.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("active transaction kept process alive past total shutdown deadline");

    assert!(!status.success(), "deadline expiry must report failure");
    assert!(started.elapsed() < Duration::from_secs(2));
    drop(active);
    // Runtime shutdown must disconnect the checked-out transaction; do not
    // release the blocker and accidentally allow a late commit to pass the test.
    // PostgreSQL may finish the current statement before noticing EOF; require
    // the actual backend to disappear, not merely the sleep to stop.
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            let connected: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)")
                    .bind(backend_pid)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if !connected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("child transaction remained active after process exit");
    let (name, revision): (String, i64) =
        sqlx::query_as("SELECT name,revision FROM identity_schools WHERE id=$1")
            .bind(school_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name, "Before shutdown");
    assert_eq!(revision, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM platform_audit_events WHERE action='school.renamed'"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

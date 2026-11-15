//! Composition root and explicit operational commands; no domain rules live here.
use crate::{
    config::Settings,
    modules::identity::{
        application::{AdminIdentity, PasswordPort, SchoolService, SessionPolicy, SessionService},
        domain::{StudentName, normalize_email},
        infrastructure::{ArgonPasswords, PgIdentityStore, PgSchoolStore, SecureTokens},
    },
    platform::{clock::SystemClock, database},
    transport::http::{
        AppState, build_router, csrf::Csrf, rate_limit::RateLimiter, state::Readiness,
    },
};
use anyhow::{Context, Result, bail};
use sqlx::PgPool;
use std::{future::IntoFuture, sync::Arc};
mod shutdown;
/// Adapter wiring is reusable in router tests and subsequent feature composition.
pub async fn assemble(pool: PgPool, settings: Settings) -> Result<AppState> {
    let identities = Arc::new(PgIdentityStore::new(pool.clone()));
    let passwords = Arc::new(ArgonPasswords::new(2));
    let sessions = SessionService::new(
        identities.clone(),
        Arc::new(SecureTokens),
        Arc::new(SystemClock),
        SessionPolicy {
            idle_seconds: settings.session.idle_seconds,
            student_absolute_seconds: settings.session.student_absolute_seconds,
            admin_absolute_seconds: settings.session.admin_absolute_seconds,
        },
    );
    let dummy_hash = passwords
        .hash("non-account-constant-time-verification-only".into())
        .await?;
    Ok(AppState {
        reports: crate::modules::reporting::application::Reports::new(Arc::new(
            crate::modules::reporting::infrastructure::PgReports::new(pool.clone()),
        )),
        bank_cms: crate::modules::assessment::admin::application::BankCms::new(Arc::new(
            crate::modules::assessment::admin::infrastructure::PgBankCms::new(pool.clone()),
        )),
        lesson_cms: crate::modules::learning::admin::application::LessonCms::new(Arc::new(
            crate::modules::learning::admin::infrastructure::PgLessonCms::new(pool.clone()),
        )),
        students: crate::modules::learning::application::StudentService::new(
            Arc::new(
                crate::modules::learning::infrastructure::PgStudentUnitOfWork::new(pool.clone()),
            ),
            sessions.clone(),
        ),
        identity: AdminIdentity::new(identities, passwords, sessions.clone(), dummy_hash),
        sessions,
        schools: SchoolService::new(Arc::new(PgSchoolStore::new(pool.clone()))),
        csrf: Csrf::new(
            settings.csrf_key.clone(),
            settings.session.secure,
            settings.http.allowed_origins.clone(),
        ),
        settings: Arc::new(settings),
        readiness: Readiness::new(pool),
        limiter: Arc::new(RateLimiter::default()),
        requests: Default::default(),
    })
}
/// Main delegates here. Commands never accept passwords as CLI arguments.
pub async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_command().await.map_err(Into::into)
}
async fn run_command() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() > 1 {
        bail!(
            "use serve, migrate, provision-admin, or init-content-config without additional arguments"
        );
    }
    let command = args.first().map(String::as_str).unwrap_or("serve");
    if !["serve", "migrate", "provision-admin", "init-content-config"].contains(&command) {
        bail!("unknown command: use serve, migrate, provision-admin, or init-content-config");
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .try_init()
        .map_err(|_| anyhow::anyhow!("telemetry initialization failed"))?;
    if command == "migrate" {
        let url = std::env::var("MIGRATION_DATABASE_URL")
            .context("MIGRATION_DATABASE_URL is required")?;
        // Do not attach SQLx connection errors: they may contain connection details.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(&url)
            .await
            .map_err(|_| anyhow::anyhow!("migration connection failed"))?;
        let result = database::MIGRATOR.run(&pool).await.map_err(|_| {
            anyhow::anyhow!("migration failed; inspect schema with the migration operator")
        });
        pool.close().await;
        result?;
        tracing::info!("migrations applied");
        return Ok(());
    }
    let settings = Settings::load()?;
    let pool = database::connect(&settings.database)
        .await
        .map_err(|_| anyhow::anyhow!("database connection failed"))?;
    if command == "init-content-config" {
        let cms = crate::modules::learning::admin::application::LessonCms::new(Arc::new(
            crate::modules::learning::admin::infrastructure::PgLessonCms::new(pool.clone()),
        ));
        let result = cms.initialize_development(&settings.environment).await;
        pool.close().await;
        result?;
        tracing::info!(
            "empty development content configuration initialized; no teaching content or credentials created"
        );
        return Ok(());
    }
    if command == "provision-admin" {
        let result = provision(pool.clone()).await;
        pool.close().await;
        result?;
        return Ok(());
    }
    // Serving owns the total shutdown budget, including pool cleanup. Never
    // append an unbounded close here after that budget has already expired.
    serve(pool, settings).await
}
async fn serve(pool: PgPool, settings: Settings) -> Result<()> {
    let bind = settings.bind;
    let grace_period = settings.shutdown_timeout;
    let state = assemble(pool.clone(), settings).await?;
    let draining = state.readiness.draining.clone();
    let requests = state.requests.clone();
    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .context("HTTP listener failed")?;
    let notify = Arc::new(tokio::sync::Notify::new());
    let signal = notify.clone();
    let server = axum::serve(
        listener,
        build_router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move { signal.notified().await })
    .into_future();
    tokio::pin!(server);
    tracing::info!(address=%bind,"HTTP server listening");
    let (completed, deadline) = tokio::select! {
        result = &mut server => (Some(result), tokio::time::Instant::now() + grace_period),
        result = shutdown::signal() => {
            result?;
            (None, tokio::time::Instant::now() + grace_period)
        }
    };
    draining.store(true, std::sync::atomic::Ordering::Relaxed);
    notify.notify_one();
    let cleanup = async {
        // Axum spawns connection tasks. A graceful serve completes only when
        // those tasks finish; dropping just this future does NOT abort them.
        let result = match completed {
            Some(result) => result,
            None => server.await,
        }
        .context("HTTP server failed");
        pool.close().await;
        result
    };
    match tokio::time::timeout_at(deadline, cleanup).await {
        Ok(result) => result,
        Err(_) => {
            requests.cancel();
            // SQLx marks the pool closed synchronously, but its returned future
            // may wait for checked-out/closing connections indefinitely. Do not
            // await it past our deadline. Request cancellation drops in-flight
            // transactions; returning failure lets main tear down the runtime,
            // including Axum connection tasks and SQLx background cleanup.
            drop(pool.close());
            bail!("HTTP shutdown deadline exceeded");
        }
    }
}
async fn provision(pool: PgPool) -> Result<()> {
    let email = normalize_email(&std::env::var("ADMIN_EMAIL").context("ADMIN_EMAIL is required")?)?;
    let name = StudentName::parse(&std::env::var("ADMIN_NAME").context("ADMIN_NAME is required")?)?;
    let password = match std::env::var("ADMIN_PASSWORD") {
        Ok(value) => value,
        Err(_) => {
            // Intended for a private pipe/secret file, not an interactive echoing prompt.
            use std::io::IsTerminal;
            if std::io::stdin().is_terminal() {
                bail!("provide ADMIN_PASSWORD privately or pipe a password to stdin");
            }
            let mut value = String::new();
            use tokio::io::AsyncReadExt;
            tokio::io::stdin()
                .take(1026)
                .read_to_string(&mut value)
                .await?;
            value.trim_end_matches(['\r', '\n']).to_owned()
        }
    };
    if !(12..=1024).contains(&password.len()) {
        bail!("admin password must contain 12 to 1024 bytes");
    }
    let schools = permission("ADMIN_CAN_MANAGE_SCHOOLS")?;
    let content = permission("ADMIN_CAN_MANAGE_CONTENT")?;
    let hash = ArgonPasswords::new(1).hash(password).await?;
    PgIdentityStore::new(pool)
        .provision_admin(
            uuid::Uuid::new_v4(),
            &email,
            name.as_str(),
            &hash,
            schools,
            content,
        )
        .await?;
    tracing::info!(
        "admin provisioned with explicit catalog/content permissions and no reporting school grants"
    );
    Ok(())
}
fn permission(key: &str) -> Result<bool> {
    match std::env::var(key).as_deref() {
        Ok("true") => Ok(true),
        Ok("false") | Err(std::env::VarError::NotPresent) => Ok(false),
        _ => bail!("{key} must be true or false"),
    }
}

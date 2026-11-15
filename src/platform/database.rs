//! One bounded pool per process and append-only embedded SQLx migrations.
use crate::{config::DatabaseSettings, shared::application::AppError};
use sqlx::{PgPool, postgres::PgPoolOptions};
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
pub async fn connect(settings: &DatabaseSettings) -> Result<PgPool, sqlx::Error> {
    let statement_timeout = settings.statement_timeout.as_millis().to_string();
    PgPoolOptions::new()
        .max_connections(settings.max_connections)
        .acquire_timeout(settings.acquire_timeout)
        .after_connect(move |connection, _| {
            let timeout = statement_timeout.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('statement_timeout',$1,false)")
                    .bind(timeout)
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SELECT set_config('lock_timeout','3000',false)")
                    .execute(&mut *connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&settings.url)
        .await
}
/// Never propagate database messages (which can include values) to the transport.
pub(crate) fn failure(error: sqlx::Error) -> AppError {
    let code = error
        .as_database_error()
        .and_then(|e| e.code())
        .map(|v| v.to_string());
    tracing::error!(sqlstate=?code,"database operation failed");
    match error {
        sqlx::Error::PoolTimedOut
        | sqlx::Error::PoolClosed
        | sqlx::Error::Io(_)
        | sqlx::Error::Tls(_) => AppError::Unavailable,
        _ => AppError::Internal,
    }
}

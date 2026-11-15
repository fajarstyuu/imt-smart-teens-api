//! PostgreSQL adapters implement core repository ports; a transaction is never independently committed.
mod assessment;
mod enrollment;
mod measurements;
mod operations;
use crate::{
    modules::learning::application::ports::*, platform::database::failure, shared::error::AppResult,
};
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Transaction};
#[derive(Clone)]
pub struct PgStudentUnitOfWork {
    pool: PgPool,
}
impl PgStudentUnitOfWork {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
pub(super) struct PgStudentTransaction {
    pub tx: Transaction<'static, Postgres>,
}
#[async_trait]
impl StudentUnitOfWork for PgStudentUnitOfWork {
    async fn begin(&self) -> AppResult<Box<dyn StudentTransaction>> {
        Ok(Box::new(PgStudentTransaction {
            tx: self.pool.begin().await.map_err(failure)?,
        }))
    }
}
#[async_trait]
impl StudentTransaction for PgStudentTransaction {
    async fn commit(self: Box<Self>) -> AppResult<()> {
        self.tx.commit().await.map_err(failure)
    }
}

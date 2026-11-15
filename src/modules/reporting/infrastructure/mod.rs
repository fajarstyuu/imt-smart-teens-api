//! Consistent read-only PostgreSQL projections; all visibility is enforced in SQL.
mod answers;
mod students;
use super::application::*;
use crate::{platform::database::failure, shared::error::AppResult};
use sqlx::{PgPool, Postgres, Transaction};
#[derive(Clone)]
pub struct PgReports {
    pool: PgPool,
}
impl PgReports {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
async fn read(pool: &PgPool) -> AppResult<Transaction<'static, Postgres>> {
    let mut tx = pool.begin().await.map_err(failure)?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(failure)?;
    Ok(tx)
}
use async_trait::async_trait;
use uuid::Uuid;
#[async_trait]
impl ReportReader for PgReports {
    async fn students(
        &self,
        a: Uuid,
        f: Filters,
        w: Window,
    ) -> AppResult<crate::shared::editing::Page<StudentSummary>> {
        students::list(&self.pool, a, f, w).await
    }
    async fn results(&self, a: Uuid, s: Uuid, e: Option<Uuid>) -> AppResult<StudentResults> {
        students::results(&self.pool, a, s, e).await
    }
    async fn attempt_answers(
        &self,
        a: Uuid,
        id: Uuid,
        w: Window,
    ) -> AppResult<crate::shared::editing::Page<QuestionAnswer>> {
        answers::attempt(&self.pool, a, id, w).await
    }
    async fn question_answers(
        &self,
        a: Uuid,
        s: QuestionSelection,
        f: Filters,
        w: Window,
    ) -> AppResult<crate::shared::editing::Page<QuestionAnswer>> {
        answers::question(&self.pool, a, s, f, w).await
    }
}

//! School mutations serialize with future registration on the same row lock.
//! Audit and catalog writes commit together; archive never deletes history.
use crate::{
    modules::identity::{
        application::{AuditActor, School, SchoolPage, SchoolQuery, SchoolStatus, SchoolStore},
        domain::SchoolName,
    },
    platform::database::failure,
    shared::application::{AppError, AppResult},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
#[derive(Clone)]
pub struct PgSchoolStore {
    pool: PgPool,
}
impl PgSchoolStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
#[derive(sqlx::FromRow)]
struct SchoolRow {
    id: Uuid,
    name: String,
    revision: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    archived_at: Option<DateTime<Utc>>,
}
impl From<SchoolRow> for School {
    fn from(r: SchoolRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            revision: r.revision,
            created_at: r.created_at,
            updated_at: r.updated_at,
            archived_at: r.archived_at,
        }
    }
}
fn write_failure(e: sqlx::Error) -> AppError {
    if e.as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        AppError::Conflict("SCHOOL_NAME_EXISTS")
    } else {
        failure(e)
    }
}
#[async_trait]
impl SchoolStore for PgSchoolStore {
    async fn list(&self, q: &SchoolQuery) -> AppResult<SchoolPage> {
        // Escape LIKE metacharacters: search is a literal substring, not a user pattern.
        let search = format!(
            "%{}%",
            q.normalized_search
                .replace(r"\", r"\\")
                .replace('%', r"\%")
                .replace('_', r"\_")
        );
        let status = match q.status {
            SchoolStatus::Active => "active",
            SchoolStatus::Archived => "archived",
            SchoolStatus::All => "all",
        };
        let mut tx = self.pool.begin().await.map_err(failure)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(failure)?;
        let total=sqlx::query_scalar("SELECT count(*) FROM identity_schools WHERE normalized_name LIKE $1 AND ($2='all' OR ($2='active' AND archived_at IS NULL) OR ($2='archived' AND archived_at IS NOT NULL))").bind(&search).bind(status).fetch_one(&mut *tx).await.map_err(failure)?;
        let rows=sqlx::query_as::<_,SchoolRow>("SELECT id,name,revision,created_at,updated_at,archived_at FROM identity_schools WHERE normalized_name LIKE $1 AND ($2='all' OR ($2='active' AND archived_at IS NULL) OR ($2='archived' AND archived_at IS NOT NULL)) ORDER BY normalized_name,id LIMIT $3 OFFSET $4").bind(search).bind(status).bind(q.limit).bind(q.offset).fetch_all(&mut *tx).await.map_err(failure)?;
        tx.commit().await.map_err(failure)?;
        Ok(SchoolPage {
            schools: rows.into_iter().map(Into::into).collect(),
            total,
        })
    }
    async fn get(&self, id: Uuid) -> AppResult<School> {
        sqlx::query_as::<_,SchoolRow>("SELECT id,name,revision,created_at,updated_at,archived_at FROM identity_schools WHERE id=$1").bind(id).fetch_optional(&self.pool).await.map_err(failure)?.map(Into::into).ok_or(AppError::NotFound)
    }
    async fn create(&self, name: SchoolName, actor: AuditActor) -> AppResult<School> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let row=sqlx::query_as::<_,SchoolRow>("INSERT INTO identity_schools(id,name,normalized_name) VALUES($1,$2,$3) RETURNING id,name,revision,created_at,updated_at,archived_at").bind(Uuid::new_v4()).bind(name.name()).bind(name.normalized()).fetch_one(&mut *tx).await.map_err(write_failure)?;
        audit(&mut tx, &actor, "school.created", &row).await?;
        tx.commit().await.map_err(failure)?;
        Ok(row.into())
    }
    async fn rename(
        &self,
        id: Uuid,
        name: SchoolName,
        expected_revision: i64,
        actor: AuditActor,
    ) -> AppResult<School> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let row = locked(&mut tx, id).await?;
        if row.revision != expected_revision {
            return Err(AppError::Conflict("SCHOOL_REVISION_CONFLICT"));
        }
        if row.archived_at.is_some() {
            return Err(AppError::Conflict("SCHOOL_ARCHIVED"));
        }
        let row=sqlx::query_as::<_,SchoolRow>("UPDATE identity_schools SET name=$2,normalized_name=$3,revision=revision+1,updated_at=clock_timestamp() WHERE id=$1 RETURNING id,name,revision,created_at,updated_at,archived_at").bind(id).bind(name.name()).bind(name.normalized()).fetch_one(&mut *tx).await.map_err(write_failure)?;
        audit(&mut tx, &actor, "school.renamed", &row).await?;
        tx.commit().await.map_err(failure)?;
        Ok(row.into())
    }
    async fn archive(
        &self,
        id: Uuid,
        expected_revision: i64,
        actor: AuditActor,
    ) -> AppResult<School> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let row = locked(&mut tx, id).await?;
        if row.revision != expected_revision {
            return Err(AppError::Conflict("SCHOOL_REVISION_CONFLICT"));
        }
        if row.archived_at.is_some() {
            tx.commit().await.map_err(failure)?;
            return Ok(row.into());
        }
        let row=sqlx::query_as::<_,SchoolRow>("UPDATE identity_schools SET archived_at=clock_timestamp(),updated_at=clock_timestamp(),revision=revision+1 WHERE id=$1 RETURNING id,name,revision,created_at,updated_at,archived_at").bind(id).fetch_one(&mut *tx).await.map_err(failure)?;
        audit(&mut tx, &actor, "school.archived", &row).await?;
        tx.commit().await.map_err(failure)?;
        Ok(row.into())
    }
}
async fn locked(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> AppResult<SchoolRow> {
    sqlx::query_as("SELECT id,name,revision,created_at,updated_at,archived_at FROM identity_schools WHERE id=$1 FOR UPDATE").bind(id).fetch_optional(&mut **tx).await.map_err(failure)?.ok_or(AppError::NotFound)
}
async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    a: &AuditActor,
    action: &str,
    row: &SchoolRow,
) -> AppResult<()> {
    sqlx::query("INSERT INTO platform_audit_events(id,actor_session_id,actor_admin_id,action,resource_type,resource_id,resource_version,request_id) VALUES($1,$2,$3,$4,'school',$5,$6,$7)").bind(Uuid::new_v4()).bind(a.session_id).bind(a.admin_id).bind(action).bind(row.id).bind(row.revision.to_string()).bind(&a.request_id).execute(&mut **tx).await.map_err(failure)?;
    Ok(())
}

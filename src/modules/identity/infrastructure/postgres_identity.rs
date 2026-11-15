//! Server-side session storage: hashes only, atomic expiry refresh/revocation.
use crate::{
    modules::identity::{
        application::{AdminCredentials, Authenticated, IdentityStore, NewSession, SessionSubject},
        domain::{AdminPrincipal, Principal},
    },
    platform::database::failure,
    shared::application::AppResult,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgIdentityStore {
    pool: PgPool,
}
impl PgIdentityStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    /// Used by explicit private CLI only. No public account-creation HTTP endpoint.
    pub async fn provision_admin(
        &self,
        id: Uuid,
        email: &str,
        name: &str,
        hash: &str,
        manage_schools: bool,
        manage_content: bool,
    ) -> AppResult<()> {
        sqlx::query("INSERT INTO identity_admins(id,email,display_name,password_hash,can_manage_schools,can_manage_content) VALUES($1,$2,$3,$4,$5,$6)").bind(id).bind(email).bind(name).bind(hash).bind(manage_schools).bind(manage_content).execute(&self.pool).await.map_err(failure)?;
        Ok(())
    }
    /// Shared transaction helper for Task 2 registration. Caller owns commit/rollback.
    pub async fn insert_in_transaction(
        tx: &mut Transaction<'_, Postgres>,
        s: &NewSession,
    ) -> AppResult<()> {
        let (role, student_id, admin_id, enrollment_id) = match s.subject {
            SessionSubject::Admin(id) => ("admin", None, Some(id), None),
            SessionSubject::Student {
                student_id,
                enrollment_id,
            } => ("student", Some(student_id), None, Some(enrollment_id)),
        };
        sqlx::query("INSERT INTO identity_sessions(id,token_hash,role,student_id,admin_id,enrollment_id,csrf_binding_hash,created_at,last_seen_at,idle_expires_at,absolute_expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$8,$9,$10)")
   .bind(s.id).bind(&s.token_hash).bind(role).bind(student_id).bind(admin_id).bind(enrollment_id).bind(&s.csrf_binding_hash).bind(s.created_at).bind(s.idle_expires_at).bind(s.absolute_expires_at).execute(&mut **tx).await.map_err(failure)?;
        Ok(())
    }
}
#[derive(sqlx::FromRow)]
struct AdminRow {
    id: Uuid,
    email: String,
    display_name: String,
    password_hash: String,
    can_manage_schools: bool,
    can_manage_content: bool,
}
async fn principal(pool: &PgPool, row: AdminRow) -> AppResult<AdminCredentials> {
    let school_ids = sqlx::query_scalar(
        "SELECT school_id FROM identity_admin_school_grants WHERE admin_id=$1 ORDER BY school_id",
    )
    .bind(row.id)
    .fetch_all(pool)
    .await
    .map_err(failure)?;
    Ok(AdminCredentials {
        password_hash: row.password_hash,
        principal: AdminPrincipal {
            id: row.id,
            email: row.email,
            name: row.display_name,
            can_manage_schools: row.can_manage_schools,
            can_manage_content: row.can_manage_content,
            school_ids,
        },
    })
}
#[derive(sqlx::FromRow)]
struct SessionRow {
    id: Uuid,
    role: String,
    student_id: Option<Uuid>,
    admin_id: Option<Uuid>,
    enrollment_id: Option<Uuid>,
    csrf_binding_hash: Vec<u8>,
}
#[async_trait]
impl IdentityStore for PgIdentityStore {
    async fn admin_by_email(&self, email: &str) -> AppResult<Option<AdminCredentials>> {
        let row=sqlx::query_as::<_,AdminRow>("SELECT id,email,display_name,password_hash,can_manage_schools,can_manage_content FROM identity_admins WHERE lower(email)=$1 AND active").bind(email).fetch_optional(&self.pool).await.map_err(failure)?;
        match row {
            Some(row) => Ok(Some(principal(&self.pool, row).await?)),
            None => Ok(None),
        }
    }
    async fn insert_session(
        &self,
        s: &NewSession,
        old_hash: Option<Vec<u8>>,
        sibling_hash: Option<Vec<u8>>,
        request_id: &str,
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        if let Some(hash) = old_hash {
            sqlx::query("UPDATE identity_sessions SET revoked_at=now() WHERE token_hash=$1 AND revoked_at IS NULL AND role='admin'").bind(hash).execute(&mut *tx).await.map_err(failure)?;
        }
        // Both independent role cookies follow the browser context rotation atomically.
        if let Some(hash) = sibling_hash {
            sqlx::query("UPDATE identity_sessions SET csrf_binding_hash=$2 WHERE token_hash=$1 AND role='student' AND revoked_at IS NULL AND idle_expires_at>now() AND absolute_expires_at>now()")
            .bind(hash).bind(&s.csrf_binding_hash).execute(&mut *tx).await.map_err(failure)?;
        }
        Self::insert_in_transaction(&mut tx, s).await?;
        if let SessionSubject::Admin(id) = s.subject {
            sqlx::query("INSERT INTO platform_audit_events(id,actor_session_id,actor_admin_id,action,resource_type,resource_id,request_id) VALUES($1,$2,$3,'admin.login','admin',$3,$4)").bind(Uuid::new_v4()).bind(s.id).bind(id).bind(request_id).execute(&mut *tx).await.map_err(failure)?;
        }
        tx.commit().await.map_err(failure)?;
        Ok(())
    }
    async fn authenticate(
        &self,
        hash: &[u8],
        now: DateTime<Utc>,
        idle_seconds: i64,
    ) -> AppResult<Option<Authenticated>> {
        // Conditional UPDATE is the expiry check and idle refresh in one statement. No
        // read/check/write gap can resurrect a revoked or absolutely expired session.
        let row=sqlx::query_as::<_,SessionRow>("UPDATE identity_sessions SET last_seen_at=$2,idle_expires_at=LEAST(absolute_expires_at,$3) WHERE token_hash=$1 AND revoked_at IS NULL AND idle_expires_at>$2 AND absolute_expires_at>$2 AND (role='student' OR EXISTS(SELECT 1 FROM identity_admins a WHERE a.id=identity_sessions.admin_id AND a.active)) RETURNING id,role,student_id,admin_id,enrollment_id,csrf_binding_hash").bind(hash).bind(now).bind(now+Duration::seconds(idle_seconds)).fetch_optional(&self.pool).await.map_err(failure)?;
        let Some(row) = row else {
            return Ok(None);
        };
        let subject = if row.role == "admin" {
            let admin=sqlx::query_as::<_,AdminRow>("SELECT id,email,display_name,password_hash,can_manage_schools,can_manage_content FROM identity_admins WHERE id=$1 AND active").bind(row.admin_id).fetch_optional(&self.pool).await.map_err(failure)?;
            let Some(admin) = admin else {
                return Ok(None);
            };
            Principal::Admin(principal(&self.pool, admin).await?.principal)
        } else {
            match (row.student_id, row.enrollment_id) {
                (Some(student_id), Some(enrollment_id)) => Principal::Student {
                    student_id,
                    enrollment_id,
                },
                _ => return Err(crate::shared::application::AppError::Internal),
            }
        };
        Ok(Some(Authenticated {
            session_id: row.id,
            principal: subject,
            csrf_binding_hash: row.csrf_binding_hash,
        }))
    }
    async fn revoke(&self, hash: &[u8], request_id: &str) -> AppResult<()> {
        let mut tx = self.pool.begin().await.map_err(failure)?;
        let row=sqlx::query_as::<_,(Uuid,Option<Uuid>)>("UPDATE identity_sessions SET revoked_at=now() WHERE token_hash=$1 AND revoked_at IS NULL RETURNING id,admin_id").bind(hash).fetch_optional(&mut *tx).await.map_err(failure)?;
        if let Some((session, Some(admin))) = row {
            sqlx::query("INSERT INTO platform_audit_events(id,actor_session_id,actor_admin_id,action,resource_type,resource_id,request_id) VALUES($1,$2,$3,'admin.logout','admin',$3,$4)").bind(Uuid::new_v4()).bind(session).bind(admin).bind(request_id).execute(&mut *tx).await.map_err(failure)?;
        }
        tx.commit().await.map_err(failure)?;
        Ok(())
    }
    async fn bind_csrf(&self, id: Uuid, binding: &[u8]) -> AppResult<()> {
        sqlx::query(
            "UPDATE identity_sessions SET csrf_binding_hash=$2 WHERE id=$1 AND revoked_at IS NULL",
        )
        .bind(id)
        .bind(binding)
        .execute(&self.pool)
        .await
        .map_err(failure)?;
        Ok(())
    }
}

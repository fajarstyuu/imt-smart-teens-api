//! Shared editorial persistence mapping and redacted audit writes.
//! Callers retain the encompassing transaction and its commit/rollback boundary.
use crate::{
    modules::identity::application::AuditActor,
    platform::database::failure,
    shared::{editing::Version, error::AppResult},
};
use sqlx::{PgConnection, Row};
use uuid::Uuid;
pub(crate) async fn audit(
    c: &mut PgConnection,
    a: &AuditActor,
    action: &str,
    kind: &str,
    id: Uuid,
    rev: i64,
) -> AppResult<()> {
    sqlx::query("INSERT INTO platform_audit_events(id,actor_session_id,actor_admin_id,action,resource_type,resource_id,resource_version,request_id,safe_metadata) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'{}')").bind(Uuid::new_v4()).bind(a.session_id).bind(a.admin_id).bind(action).bind(kind).bind(id).bind(rev.to_string()).bind(&a.request_id).execute(c).await.map_err(failure)?;
    Ok(())
}
pub(crate) async fn versions(c: &mut PgConnection, sql: &str, id: Uuid) -> AppResult<Vec<Version>> {
    let rows = sqlx::query(sql)
        .bind(id)
        .fetch_all(c)
        .await
        .map_err(failure)?;
    rows.into_iter()
        .map(|r| {
            Ok(Version {
                id: r.try_get("id").map_err(failure)?,
                version: r.try_get("version").map_err(failure)?,
                published_at: r.try_get("published_at").map_err(failure)?,
            })
        })
        .collect()
}

//! Transactional operation reservation and safe response caching; raw cookies never enter JSON.
use super::PgStudentTransaction;
use crate::{
    modules::learning::application::ports::*, platform::database::failure, shared::error::AppResult,
};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;
#[async_trait]
impl OperationRepository for PgStudentTransaction {
    async fn operation(
        &mut self,
        scope: &[u8],
        operation: &str,
        key: &str,
    ) -> AppResult<Option<CachedOperation>> {
        // Domain-separated length-framed hash avoids ambiguous concatenation. Hash collisions only
        // serialize unrelated work; the full relational key still decides replay/conflict.
        let mut hash = Sha256::new();
        for bytes in [scope, operation.as_bytes(), key.as_bytes()] {
            hash.update((bytes.len() as u64).to_be_bytes());
            hash.update(bytes);
        }
        let digest = hash.finalize();
        let mut prefix = [0u8; 8];
        prefix.copy_from_slice(&digest[..8]);
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(i64::from_be_bytes(prefix))
            .execute(&mut *self.tx)
            .await
            .map_err(failure)?;
        sqlx::query("DELETE FROM platform_idempotency WHERE scope_hash=$1 AND operation=$2 AND key=$3 AND expires_at<=now()").bind(scope).bind(operation).bind(key).execute(&mut *self.tx).await.map_err(failure)?;
        let row=sqlx::query("SELECT payload_hash,status_code,response_body FROM platform_idempotency WHERE scope_hash=$1 AND operation=$2 AND key=$3").bind(scope).bind(operation).bind(key).fetch_optional(&mut *self.tx).await.map_err(failure)?;
        row.map(|r| {
            Ok(CachedOperation {
                payload_hash: r.try_get("payload_hash").map_err(failure)?,
                result: super::cache::decode(
                    r.try_get("status_code").map_err(failure)?,
                    r.try_get("response_body").map_err(failure)?,
                )?,
            })
        })
        .transpose()
    }
    async fn remember(
        &mut self,
        scope: &[u8],
        operation: &str,
        key: &str,
        v: CachedOperation,
    ) -> AppResult<()> {
        let (status, body) = super::cache::encode(v.result)?;
        sqlx::query("INSERT INTO platform_idempotency(scope_hash,operation,key,payload_hash,status_code,response_body,expires_at) VALUES($1,$2,$3,$4,$5,$6,now()+interval '24 hours')").bind(scope).bind(operation).bind(key).bind(v.payload_hash).bind(status).bind(body).execute(&mut *self.tx).await.map_err(failure)?;
        Ok(())
    }
    async fn audit(&mut self, action: &str, resource: Uuid, request_id: &str) -> AppResult<()> {
        sqlx::query("INSERT INTO platform_audit_events(id,action,resource_type,resource_id,request_id) VALUES($1,$2,'student_workflow',$3,$4)").bind(Uuid::new_v4()).bind(action).bind(resource).bind(request_id).execute(&mut *self.tx).await.map_err(failure)?;
        Ok(())
    }
}

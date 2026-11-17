//! Verified content permission and audit actor construction shared by editor use cases.
use super::AuditActor;
use crate::{modules::identity::domain::Principal, shared::error::AppResult};
use uuid::Uuid;
pub fn authorize_content(p: &Principal) -> AppResult<()> {
    // Identity, not a legacy content flag, grants the global admin capability.
    p.admin()?;
    Ok(())
}
pub fn content_actor(p: &Principal, session_id: Uuid, request_id: String) -> AppResult<AuditActor> {
    authorize_content(p)?;
    Ok(AuditActor {
        admin_id: p.admin()?.id,
        session_id,
        request_id,
    })
}

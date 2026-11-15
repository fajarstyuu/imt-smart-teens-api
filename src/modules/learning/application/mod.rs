//! Student journey use cases. Every operation owns a single unit-of-work transaction.
mod lessons;
pub mod ports;
use crate::{
    modules::identity::application::SessionService,
    shared::error::{AppError, AppResult},
};
use ports::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
#[derive(Clone)]
pub struct StudentService {
    pub(crate) uow: Arc<dyn StudentUnitOfWork>,
    pub(crate) sessions: SessionService,
}
impl StudentService {
    pub fn new(uow: Arc<dyn StudentUnitOfWork>, sessions: SessionService) -> Self {
        Self { uow, sessions }
    }
}
/// Validated idempotency input; no raw cookies, passwords or session tokens.
pub struct Operation {
    pub scope: Vec<u8>,
    pub key: String,
    pub payload_hash: Vec<u8>,
    pub request_id: String,
}
impl Operation {
    pub fn new(
        scope: Vec<u8>,
        key: String,
        payload: &impl serde::Serialize,
        request_id: String,
    ) -> AppResult<Self> {
        if key.is_empty() || key.len() > 128 || !key.bytes().all(|b| (32..=126).contains(&b)) {
            return Err(AppError::Invalid("INVALID_IDEMPOTENCY_KEY"));
        }
        Ok(Self {
            scope,
            key,
            payload_hash: Sha256::digest(
                serde_json::to_vec(payload).map_err(|_| AppError::Internal)?,
            )
            .to_vec(),
            request_id,
        })
    }
    pub async fn replay(
        &self,
        tx: &mut dyn StudentTransaction,
        name: &str,
    ) -> AppResult<Option<(u16, Value)>> {
        match tx.operation(&self.scope, name, &self.key).await? {
            Some(cached) if cached.payload_hash != self.payload_hash => {
                Err(AppError::Conflict("IDEMPOTENCY_CONFLICT"))
            }
            Some(cached) => Ok(Some((cached.status, cached.body))),
            None => Ok(None),
        }
    }
    pub async fn remember(
        &self,
        tx: &mut dyn StudentTransaction,
        name: &str,
        status: u16,
        body: Value,
    ) -> AppResult<()> {
        tx.remember(
            &self.scope,
            name,
            &self.key,
            CachedOperation {
                payload_hash: self.payload_hash.clone(),
                status,
                body,
            },
        )
        .await
    }
}

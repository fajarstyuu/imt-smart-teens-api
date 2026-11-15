//! Cheap cloneable transport state; handlers receive facades rather than repositories.
use super::{csrf::Csrf, rate_limit::RateLimiter};
use crate::{
    config::Settings,
    modules::identity::application::{AdminIdentity, SchoolService, SessionService},
};
use sqlx::PgPool;
use std::sync::{Arc, atomic::AtomicBool};
#[derive(Clone)]
pub struct AppState {
    pub identity: AdminIdentity,
    pub sessions: SessionService,
    pub schools: SchoolService,
    pub csrf: Csrf,
    pub settings: Arc<Settings>,
    pub readiness: Readiness,
    pub limiter: Arc<RateLimiter>,
}
/// The pool is isolated to health infrastructure, not available as a handler shortcut.
#[derive(Clone)]
pub struct Readiness {
    pool: PgPool,
    pub draining: Arc<AtomicBool>,
}
impl Readiness {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            draining: Arc::new(AtomicBool::new(false)),
        }
    }
    pub async fn check(&self) -> bool {
        if self.draining.load(std::sync::atomic::Ordering::Relaxed) {
            return false;
        }
        matches!(
            tokio::time::timeout(
                std::time::Duration::from_secs(1),
                sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&self.pool)
            )
            .await,
            Ok(Ok(1))
        )
    }
}

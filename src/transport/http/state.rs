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
    pub bank_cms: crate::modules::assessment::admin::application::BankCms,
    pub lesson_cms: crate::modules::learning::admin::application::LessonCms,
    pub students: crate::modules::learning::application::StudentService,
    pub identity: AdminIdentity,
    pub sessions: SessionService,
    pub schools: SchoolService,
    pub csrf: Csrf,
    pub settings: Arc<Settings>,
    pub readiness: Readiness,
    pub limiter: Arc<RateLimiter>,
    pub requests: RequestCancellation,
}

/// Sticky broadcast cancellation for HTTP work at the total shutdown deadline.
/// Receivers created after cancellation must also stop; a bare Notify would
/// lose that signal. Clones share one watch sender without spawning tasks.
#[derive(Clone)]
pub struct RequestCancellation(tokio::sync::watch::Sender<bool>);
impl Default for RequestCancellation {
    fn default() -> Self {
        Self(tokio::sync::watch::channel(false).0)
    }
}
impl RequestCancellation {
    /// Stop existing and subsequent request futures, allowing transaction drop.
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }

    /// Wait until cancellation, including when it preceded subscription.
    pub async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
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

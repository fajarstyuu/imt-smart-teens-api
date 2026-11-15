//! Clock contract shared by session issuance and future transactional workflows.
use chrono::{DateTime, Utc};
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

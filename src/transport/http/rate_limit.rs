//! Bounded, per-instance IP limits. A multi-replica deployment needs a shared limiter.
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};
pub struct RateLimiter {
    entries: Mutex<HashMap<(IpAddr, bool), (Instant, u32)>>,
}
impl Default for RateLimiter {
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}
impl RateLimiter {
    pub fn allow(&self, ip: IpAddr, login: bool) -> bool {
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        let now = Instant::now();
        entries.retain(|_, (start, _)| now.duration_since(*start) < Duration::from_secs(60));
        if entries.len() >= 4096 && !entries.contains_key(&(ip, login)) {
            return false;
        }
        let entry = entries.entry((ip, login)).or_insert((now, 0));
        entry.1 += 1;
        entry.1 <= if login { 10 } else { 300 }
    }
}

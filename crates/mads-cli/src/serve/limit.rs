use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

/// A small in-process rate limiter. The studio is single-user, so one global
/// bucket is enough to stop a runaway tab or a script from starting hundreds
/// of jobs.
#[derive(Default)]
pub struct RateLimiter {
    inner: Mutex<HashMap<String, Bucket>>,
}

struct Bucket {
    started: Instant,
    count: u32,
}

impl RateLimiter {
    pub fn allow(&self, key: &str, limit: u32, window: Duration) -> bool {
        let Ok(mut buckets) = self.inner.lock() else {
            return false;
        };
        let now = Instant::now();
        let bucket = buckets.entry(key.to_string()).or_insert(Bucket {
            started: now,
            count: 0,
        });
        if now.duration_since(bucket.started) >= window {
            bucket.started = now;
            bucket.count = 0;
        }
        if bucket.count >= limit {
            return false;
        }
        bucket.count += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limit_is_enforced() {
        let limiter = RateLimiter::default();
        let window = Duration::from_secs(60);
        assert!(limiter.allow("a", 2, window));
        assert!(limiter.allow("a", 2, window));
        assert!(!limiter.allow("a", 2, window));
        assert!(limiter.allow("b", 2, window));
    }

    #[test]
    fn the_window_resets_the_count() {
        let limiter = RateLimiter::default();
        assert!(limiter.allow("a", 1, Duration::from_millis(0)));
        assert!(limiter.allow("a", 1, Duration::from_millis(0)));
    }
}

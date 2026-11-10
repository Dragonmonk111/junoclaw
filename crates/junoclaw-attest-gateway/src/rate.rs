//! Per-customer token-bucket rate limiter (in-memory, no deps).

use std::collections::HashMap;
use std::time::Instant;

use parking_lot::Mutex;

pub struct RateLimiter {
    rps: u32,
    buckets: Mutex<HashMap<String, Bucket>>,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(rps: u32) -> Self {
        Self {
            rps,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Consume one token for `customer`. Returns false when over limit.
    /// rps=0 means unlimited.
    pub fn allow(&self, customer: &str) -> bool {
        if self.rps == 0 {
            return true;
        }
        let mut map = self.buckets.lock();
        let b = map.entry(customer.to_string()).or_insert(Bucket {
            tokens: self.rps as f64,
            last: Instant::now(),
        });
        let elapsed = b.last.elapsed().as_secs_f64();
        b.tokens = (b.tokens + elapsed * self.rps as f64).min(self.rps as f64);
        b.last = Instant::now();
        if b.tokens >= 1.0 {
            b.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

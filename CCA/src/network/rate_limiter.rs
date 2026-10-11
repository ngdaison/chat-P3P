// src/network/rate_limiter.rs
//! Token Bucket rate limiting and backpressure control for peer network traffic.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Instant;

/// Individual Token Bucket for a single client or peer.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    tokens: f64,
    capacity: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Attempts to consume `cost` tokens from the bucket.
    /// Returns true if successful, false if the rate limit is exceeded.
    pub fn try_consume(&mut self, cost: f64) -> bool {
        self.refill();
        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }

    pub fn current_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }
}

/// Thread-safe rate limiter managing token buckets for remote network addresses.
pub struct RateLimiter {
    buckets: Mutex<HashMap<SocketAddr, TokenBucket>>,
    default_capacity: f64,
    default_refill_rate: f64,
}

impl RateLimiter {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            default_capacity: capacity,
            default_refill_rate: refill_rate,
        }
    }

    /// Checks if a request from `addr` with a cost of `cost` is allowed.
    pub fn check_allowed(&self, addr: &SocketAddr, cost: f64) -> bool {
        let mut buckets = self.buckets.lock().unwrap();
        let bucket = buckets
            .entry(*addr)
            .or_insert_with(|| TokenBucket::new(self.default_capacity, self.default_refill_rate));
        bucket.try_consume(cost)
    }

    /// Evicts inactive entries to prevent memory bloat.
    pub fn cleanup_inactive(&self) {
        let mut buckets = self.buckets.lock().unwrap();
        let now = Instant::now();
        buckets.retain(|_, b| now.duration_since(b.last_refill).as_secs() < 300);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn test_token_bucket_consume_and_refill() {
        let mut bucket = TokenBucket::new(10.0, 50.0); // 10 capacity, 50/sec refill

        // Consume 8 tokens -> should succeed
        assert!(bucket.try_consume(8.0));
        assert!(bucket.current_tokens() <= 2.1);

        // Consume 5 tokens -> should fail (only ~2 available)
        assert!(!bucket.try_consume(5.0));

        // Sleep 100ms -> should refill ~5 tokens
        sleep(Duration::from_millis(100));
        assert!(bucket.try_consume(5.0));
    }

    #[test]
    fn test_rate_limiter_per_addr() {
        let limiter = RateLimiter::new(5.0, 10.0);
        let addr1: SocketAddr = "127.0.0.1:8001".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:8002".parse().unwrap();

        // Consume all tokens for addr1
        for _ in 0..5 {
            assert!(limiter.check_allowed(&addr1, 1.0));
        }
        // Next should fail for addr1
        assert!(!limiter.check_allowed(&addr1, 1.0));

        // But addr2 has its own bucket and should still succeed
        assert!(limiter.check_allowed(&addr2, 1.0));
    }
}

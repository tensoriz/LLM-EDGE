use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
pub struct ProviderStats {
    pub request_count: AtomicU64,
    pub error_count: AtomicU64,
    // Latency stored as microseconds to allow atomic operations
    pub p50_latency_us: AtomicU64, 
    pub p99_latency_us: AtomicU64,
    // EWMA of latency (microseconds)
    pub ewma_latency_us: AtomicU64,
    pub consec_errors: AtomicU32,
}

impl ProviderStats {
    pub fn new() -> Self {
        Self {
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            p50_latency_us: AtomicU64::new(0),
            p99_latency_us: AtomicU64::new(0),
            ewma_latency_us: AtomicU64::new(0),
            consec_errors: AtomicU32::new(0),
        }
    }

    pub fn record_success(&self, latency: Duration) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
        self.consec_errors.store(0, Ordering::Relaxed);
        
        let latency_us = latency.as_micros() as u64;
        
        // Update EWMA: New = Alpha * sample + (1 - Alpha) * Old
        // Using Alpha = 0.2 approx? For simple atomic, we might need a spin loop or just relaxed approximation.
        // Let's implement a simple relaxed update for now. 
        // This is a simplification; for strict EWMA we need f64 or fixed point arithmetic.
        // Here we use integer math: new_avg = (old_avg * 7 + new_val) / 8  (Alpha = 1/8 = 0.125)
        
        let mut old = self.ewma_latency_us.load(Ordering::Relaxed);
        loop {
            let new_val = if old == 0 {
                 latency_us 
            } else {
                (old * 7 + latency_us) / 8
            };
            
            match self.ewma_latency_us.compare_exchange_weak(old, new_val, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(x) => old = x,
            }
        }
    }

    pub fn record_failure(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        self.consec_errors.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn score(&self) -> f64 {
        // Lower is better.
        // Score = EWMA_Latency * (1 + Error_Rate_Penalty)
        // Simplistic example.
        let l = self.ewma_latency_us.load(Ordering::Relaxed) as f64;
        let e = self.consec_errors.load(Ordering::Relaxed) as f64;
        
        // Massive penalty for consecutive errors to trigger circuit breaking logic elsewhere
        l * (1.0 + e * 10.0) 
    }
}

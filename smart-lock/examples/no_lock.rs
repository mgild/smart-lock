//! Demonstrates `#[no_lock]` for self-synchronized fields.
//!
//! Fields marked `#[no_lock]` are stored as bare `T` (not wrapped in `RwLock`)
//! and always accessible as `&T` on the guard — no lock mode needed.
//!
//! Run with: `cargo run --example no_lock`

use smart_lock::smart_lock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[smart_lock]
struct ApiState {
    /// Cached response data — needs locking for safe mutation.
    cache: Vec<String>,

    /// Request counter — already thread-safe, no lock needed.
    #[no_lock]
    request_count: AtomicU64,
}

#[tokio::main]
async fn main() {
    let state = Arc::new(ApiStateLock::new(vec![], AtomicU64::new(0)));

    let mut handles = vec![];

    // Spawn readers that bump the counter without locking cache
    for _ in 0..4 {
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..1000 {
                let guard = s.builder().read_cache().lock().await;
                // request_count is always accessible — no lock mode needed
                guard.request_count.fetch_add(1, Ordering::Relaxed);
                let _len = guard.cache.len();
            }
        }));
    }

    // Spawn a writer that updates the cache
    {
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..100 {
                let mut guard = s.builder().write_cache().lock().await;
                guard.cache.push(format!("item-{}", i));
                // Still accessible while holding a write lock on cache
                guard.request_count.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let guard = state.lock_all().await;
    println!("Cache size: {}", guard.cache.len());
    println!(
        "Total requests: {}",
        guard.request_count.load(Ordering::Relaxed)
    );
}

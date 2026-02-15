//! A concurrent session store demonstrating smart-lock's value.
//!
//! The session store has three fields:
//! - `sessions`: the actual session data (HashMap)
//! - `stats`: access statistics (read frequently, written rarely)
//! - `config`: runtime configuration (read by everyone, written by admin)
//!
//! With a single `RwLock<SessionStore>`, updating stats would block config readers.
//! With smart-lock, each field is independently lockable — a stats write doesn't
//! block a config read.
//!
//! Run with: `cargo run --example session_store`

use smart_lock::smart_lock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Session {
    user_id: u64,
    token: String,
    expires_at: u64,
}

#[derive(Debug, Clone)]
struct Stats {
    total_lookups: u64,
    cache_hits: u64,
    cache_misses: u64,
}

#[derive(Debug, Clone)]
struct Config {
    max_sessions: usize,
    session_ttl_secs: u64,
}

#[smart_lock]
struct SessionStore {
    sessions: HashMap<String, Session>,
    stats: Stats,
    config: Config,
}

#[tokio::main]
async fn main() {
    let store = Arc::new(SessionStoreLock::new(
        HashMap::new(),
        Stats {
            total_lookups: 0,
            cache_hits: 0,
            cache_misses: 0,
        },
        Config {
            max_sessions: 10_000,
            session_ttl_secs: 3600,
        },
    ));

    let mut handles = vec![];

    // Spawn 4 "lookup" tasks — read sessions + write stats concurrently
    for task_id in 0..4u64 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..1000 {
                let token = format!("token-{}-{}", task_id, i % 10);

                // Read sessions and write stats — config is untouched
                let mut guard = s
                    .builder()
                    .read_sessions()
                    .write_stats()
                    .lock()
                    .await;

                guard.stats.total_lookups += 1;
                if guard.sessions.contains_key(&token) {
                    guard.stats.cache_hits += 1;
                } else {
                    guard.stats.cache_misses += 1;
                }
            }
        }));
    }

    // Spawn 2 "create session" tasks — write sessions, read config
    for task_id in 0..2u64 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..100 {
                let token = format!("token-{}-{}", task_id, i);

                // Write sessions + read config (check max_sessions)
                // Stats are untouched — doesn't block lookup tasks' stats writes
                let mut guard = s
                    .builder()
                    .write_sessions()
                    .read_config()
                    .lock()
                    .await;

                if guard.sessions.len() < guard.config.max_sessions {
                    guard.sessions.insert(
                        token,
                        Session {
                            user_id: task_id * 1000 + i,
                            token: format!("secret-{}", i),
                            expires_at: 9999999,
                        },
                    );
                }
            }
        }));
    }

    // Spawn 1 "admin config update" task — only touches config
    {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..10 {
                // Only locks config — sessions and stats are completely unblocked
                let mut guard = s.builder().write_config().lock().await;
                guard.config.session_ttl_secs += 60;
                tokio::task::yield_now().await;
            }
        }));
    }

    // Spawn 4 "monitoring" tasks — read stats + read config (never blocks writers on sessions)
    for _ in 0..4 {
        let s = store.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let guard = s.builder().read_stats().read_config().lock().await;
                let _hit_rate = if guard.stats.total_lookups > 0 {
                    guard.stats.cache_hits as f64 / guard.stats.total_lookups as f64
                } else {
                    0.0
                };
                let _ttl = guard.config.session_ttl_secs;
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    // Final read using lock_all
    let guard = store.lock_all().await;
    println!("Sessions: {}", guard.sessions.len());
    println!(
        "Stats: {} lookups, {} hits, {} misses",
        guard.stats.total_lookups, guard.stats.cache_hits, guard.stats.cache_misses
    );
    println!(
        "Config: max={}, ttl={}s",
        guard.config.max_sessions, guard.config.session_ttl_secs
    );

    // Demonstrate into_inner: extract the data when done
    drop(guard);
    let data = store.lock_all().await;
    println!(
        "\nHit rate: {:.1}%",
        if data.stats.total_lookups > 0 {
            data.stats.cache_hits as f64 / data.stats.total_lookups as f64 * 100.0
        } else {
            0.0
        }
    );
}

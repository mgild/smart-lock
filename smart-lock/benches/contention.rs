use criterion::{criterion_group, criterion_main, Criterion};
use smart_lock::smart_lock;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

// === Shared state for all approaches ===

// Approach 1: Single RwLock
struct SingleLockState {
    a: u64,
    b: u64,
    c: u64,
    d: u64,
}

// Approach 2: Manual per-field
struct ManualState {
    a: RwLock<u64>,
    b: RwLock<u64>,
    c: RwLock<u64>,
    d: RwLock<u64>,
}

impl ManualState {
    fn new() -> Self {
        Self {
            a: RwLock::new(0),
            b: RwLock::new(0),
            c: RwLock::new(0),
            d: RwLock::new(0),
        }
    }
}

// Approach 3: smart-lock
#[smart_lock]
struct SmartState {
    a: u64,
    b: u64,
    c: u64,
    d: u64,
}

const TASKS: usize = 8;
const OPS_PER_TASK: usize = 1000;

fn bench_write_contention(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("write_contention");

    // Single RwLock -- all writers serialize on the whole struct
    group.bench_function("single_rwlock", |b| {
        b.to_async(&rt).iter(|| async {
            let state = Arc::new(RwLock::new(SingleLockState {
                a: 0,
                b: 0,
                c: 0,
                d: 0,
            }));
            let mut handles = vec![];
            for field_idx in 0..4u64 {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..OPS_PER_TASK {
                        let mut guard = s.write().await;
                        match field_idx {
                            0 => guard.a += 1,
                            1 => guard.b += 1,
                            2 => guard.c += 1,
                            _ => guard.d += 1,
                        }
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        });
    });

    // Manual per-field -- each writer locks only its field
    group.bench_function("manual_per_field", |b| {
        b.to_async(&rt).iter(|| async {
            let state = Arc::new(ManualState::new());
            let mut handles = vec![];
            for field_idx in 0..4u64 {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..OPS_PER_TASK {
                        match field_idx {
                            0 => *s.a.write().await += 1,
                            1 => *s.b.write().await += 1,
                            2 => *s.c.write().await += 1,
                            _ => *s.d.write().await += 1,
                        }
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        });
    });

    // Smart-lock -- generated per-field locking
    group.bench_function("smart_lock", |b| {
        b.to_async(&rt).iter(|| async {
            let state = Arc::new(SmartStateLock::new(0, 0, 0, 0));
            let mut handles = vec![];
            for field_idx in 0..4u64 {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..OPS_PER_TASK {
                        match field_idx {
                            0 => {
                                let mut g = s.builder().write_a().lock().await;
                                *g.a += 1;
                            }
                            1 => {
                                let mut g = s.builder().write_b().lock().await;
                                *g.b += 1;
                            }
                            2 => {
                                let mut g = s.builder().write_c().lock().await;
                                *g.c += 1;
                            }
                            _ => {
                                let mut g = s.builder().write_d().lock().await;
                                *g.d += 1;
                            }
                        }
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        });
    });

    group.finish();
}

fn bench_read_heavy(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("read_heavy");

    // Single RwLock
    group.bench_function("single_rwlock", |b| {
        b.to_async(&rt).iter(|| async {
            let state = Arc::new(RwLock::new(SingleLockState {
                a: 42,
                b: 42,
                c: 42,
                d: 42,
            }));
            let mut handles = vec![];
            for _i in 0..TASKS {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for j in 0..OPS_PER_TASK {
                        if j % 10 == 0 {
                            let mut guard = s.write().await;
                            guard.a += 1;
                        } else {
                            let guard = s.read().await;
                            std::hint::black_box(guard.a + guard.b);
                        }
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        });
    });

    // Manual per-field
    group.bench_function("manual_per_field", |b| {
        b.to_async(&rt).iter(|| async {
            let state = Arc::new(ManualState {
                a: RwLock::new(42),
                b: RwLock::new(42),
                c: RwLock::new(42),
                d: RwLock::new(42),
            });
            let mut handles = vec![];
            for _i in 0..TASKS {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for j in 0..OPS_PER_TASK {
                        if j % 10 == 0 {
                            *s.a.write().await += 1;
                        } else {
                            let a = s.a.read().await;
                            let b = s.b.read().await;
                            std::hint::black_box(*a + *b);
                        }
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        });
    });

    // Smart-lock
    group.bench_function("smart_lock", |b| {
        b.to_async(&rt).iter(|| async {
            let state = Arc::new(SmartStateLock::new(42, 42, 42, 42));
            let mut handles = vec![];
            for _i in 0..TASKS {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for j in 0..OPS_PER_TASK {
                        if j % 10 == 0 {
                            let mut g = s.builder().write_a().lock().await;
                            *g.a += 1;
                        } else {
                            let g = s.builder().read_a().read_b().lock().await;
                            std::hint::black_box(*g.a + *g.b);
                        }
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        });
    });

    group.finish();
}

fn bench_mixed_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("mixed_access");

    // Single RwLock -- writers block all readers
    group.bench_function("single_rwlock", |b| {
        b.to_async(&rt).iter(|| async {
            let state = Arc::new(RwLock::new(SingleLockState {
                a: 0,
                b: 100,
                c: 0,
                d: 0,
            }));
            let mut handles = vec![];
            // 4 writers on field a
            for _ in 0..4 {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..OPS_PER_TASK {
                        let mut guard = s.write().await;
                        guard.a += 1;
                    }
                }));
            }
            // 4 readers on field b
            for _ in 0..4 {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..OPS_PER_TASK {
                        let guard = s.read().await;
                        std::hint::black_box(guard.b);
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        });
    });

    // Manual per-field -- writers on a don't block readers on b
    group.bench_function("manual_per_field", |b| {
        b.to_async(&rt).iter(|| async {
            let state = Arc::new(ManualState {
                a: RwLock::new(0),
                b: RwLock::new(100),
                c: RwLock::new(0),
                d: RwLock::new(0),
            });
            let mut handles = vec![];
            for _ in 0..4 {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..OPS_PER_TASK {
                        *s.a.write().await += 1;
                    }
                }));
            }
            for _ in 0..4 {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..OPS_PER_TASK {
                        std::hint::black_box(*s.b.read().await);
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        });
    });

    // Smart-lock
    group.bench_function("smart_lock", |b| {
        b.to_async(&rt).iter(|| async {
            let state = Arc::new(SmartStateLock::new(0, 100, 0, 0));
            let mut handles = vec![];
            for _ in 0..4 {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..OPS_PER_TASK {
                        let mut g = s.builder().write_a().lock().await;
                        *g.a += 1;
                    }
                }));
            }
            for _ in 0..4 {
                let s = state.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..OPS_PER_TASK {
                        let g = s.builder().read_b().lock().await;
                        std::hint::black_box(*g.b);
                    }
                }));
            }
            for h in handles {
                h.await.unwrap();
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_write_contention,
    bench_read_heavy,
    bench_mixed_access
);
criterion_main!(benches);

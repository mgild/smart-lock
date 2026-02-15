use smart_lock::smart_lock;
use std::sync::Arc;

#[smart_lock]
struct Shared {
    x: u64,
    y: u64,
}

#[tokio::test]
async fn multiple_readers_concurrent() {
    let state = Arc::new(SharedLock::new(42, 99));

    let mut handles = vec![];
    for _ in 0..10 {
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            let guard = s.lock_all().await;
            assert_eq!(*guard.x, 42);
            assert_eq!(*guard.y, 99);
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}

#[tokio::test]
async fn writer_excludes_readers() {
    let state = Arc::new(SharedLock::new(0, 0));

    let s = state.clone();
    let writer = tokio::spawn(async move {
        for _ in 0..1000 {
            let mut guard = s.builder().write_x().lock().await;
            *guard.x += 1;
        }
    });

    writer.await.unwrap();

    let guard = state.lock_all().await;
    assert_eq!(*guard.x, 1000);
}

#[tokio::test]
async fn different_fields_writable_concurrently() {
    let state = Arc::new(SharedLock::new(0, 0));

    let s1 = state.clone();
    let s2 = state.clone();

    let h1 = tokio::spawn(async move {
        for _ in 0..1000 {
            let mut guard = s1.builder().write_x().lock().await;
            *guard.x += 1;
        }
    });

    let h2 = tokio::spawn(async move {
        for _ in 0..1000 {
            let mut guard = s2.builder().write_y().lock().await;
            *guard.y += 1;
        }
    });

    h1.await.unwrap();
    h2.await.unwrap();

    let guard = state.lock_all().await;
    assert_eq!(*guard.x, 1000);
    assert_eq!(*guard.y, 1000);
}

#[tokio::test]
async fn concurrent_mixed_access() {
    let state = Arc::new(SharedLock::new(0, 100));

    let mut handles = vec![];

    // 5 writers on x
    for _ in 0..5 {
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let mut guard = s.builder().write_x().lock().await;
                *guard.x += 1;
            }
        }));
    }

    // 5 readers on y
    for _ in 0..5 {
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let guard = s.builder().read_y().lock().await;
                assert_eq!(*guard.y, 100);
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let guard = state.lock_all().await;
    assert_eq!(*guard.x, 500);
    assert_eq!(*guard.y, 100);
}

#[tokio::test]
async fn upgrade_under_contention() {
    let state = Arc::new(SharedLock::new(0, 0));

    let mut handles = vec![];

    // 5 tasks: acquire upgradable read, read, upgrade, write
    for _ in 0..5 {
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let guard = s.builder().upgrade_x().lock().await;
                let _val = *guard.x; // read while upgradable
                let mut guard = guard.upgrade_x().await;
                *guard.x += 1;
            }
        }));
    }

    // Concurrent readers on y (uncontested)
    for _ in 0..5 {
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..100 {
                let guard = s.builder().read_y().lock().await;
                assert_eq!(*guard.y, 0);
            }
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let guard = state.lock_all().await;
    assert_eq!(*guard.x, 500);
}

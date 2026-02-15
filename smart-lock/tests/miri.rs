//! Miri-compatible tests using pollster::block_on instead of tokio.
//! Tokio's IO driver uses kqueue/epoll syscalls that miri can't handle.

use smart_lock::smart_lock;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    pollster::block_on(f)
}

#[smart_lock]
struct MyState {
    counter: u32,
    name: String,
    data: Vec<u8>,
}

#[test]
fn create_and_lock_all_fields_mut() {
    block_on(async {
        let state = MyStateLock::new(0, "hello".into(), vec![1, 2, 3]);
        let mut guard = state.builder().write_counter().write_name().write_data().lock().await;
        *guard.counter = 42;
        *guard.name = "world".into();
        guard.data.push(4);
        assert_eq!(*guard.counter, 42);
        assert_eq!(&*guard.name, "world");
        assert_eq!(&*guard.data, &[1, 2, 3, 4]);
    });
}

#[test]
fn mixed_read_and_write() {
    block_on(async {
        let state = MyStateLock::new(10, "test".into(), vec![]);
        let mut guard = state.builder().write_counter().read_name().lock().await;
        *guard.counter += 5;
        assert_eq!(*guard.counter, 15);
        assert_eq!(&*guard.name, "test");
    });
}

#[test]
fn mutation_persists_across_locks() {
    block_on(async {
        let state = MyStateLock::new(0, String::new(), vec![]);
        {
            let mut guard = state.builder().write_counter().lock().await;
            *guard.counter = 100;
        }
        let guard = state.builder().read_counter().lock().await;
        assert_eq!(*guard.counter, 100);
    });
}

#[test]
fn builder_selective_fields() {
    block_on(async {
        let state = MyStateLock::new(100, "selective".into(), vec![9]);
        let guard = state.builder().read_counter().read_name().lock().await;
        assert_eq!(*guard.counter, 100);
        assert_eq!(&*guard.name, "selective");
    });
}

#[test]
fn into_inner_reconstructs_original() {
    let state = MyStateLock::new(42, "hello".into(), vec![1, 2, 3]);
    let original: MyState = state.into_inner();
    assert_eq!(original.counter, 42);
    assert_eq!(original.name, "hello");
    assert_eq!(original.data, vec![1, 2, 3]);
}

#[test]
fn from_impl() {
    let original = MyState { counter: 99, name: "from".into(), data: vec![1] };
    let state: MyStateLock = original.into();
    let back = state.into_inner();
    assert_eq!(back.counter, 99);
}

#[test]
fn get_mut_per_field() {
    let mut state = MyStateLock::new(0, "hello".into(), vec![]);
    *state.get_mut_counter() = 42;
    *state.get_mut_name() = "mutated".into();
    state.get_mut_data().push(1);
    assert_eq!(*state.get_mut_counter(), 42);
    assert_eq!(state.get_mut_name().as_str(), "mutated");
    assert_eq!(state.get_mut_data(), &vec![1]);
}

#[test]
fn upgrade_read_to_write() {
    block_on(async {
        let state = MyStateLock::new(0, String::new(), vec![]);
        let guard = state.builder().upgrade_counter().lock().await;
        assert_eq!(*guard.counter, 0);
        let mut guard = guard.upgrade_counter().await;
        *guard.counter = 42;
        assert_eq!(*guard.counter, 42);
    });
}

#[test]
fn downgrade_write_to_read() {
    block_on(async {
        let state = MyStateLock::new(0, String::new(), vec![]);
        let mut guard = state.builder().write_counter().lock().await;
        *guard.counter = 42;
        let guard = guard.downgrade_counter();
        assert_eq!(*guard.counter, 42);
    });
}

#[test]
fn relock_drops_and_rebuilds() {
    block_on(async {
        let state = MyStateLock::new(0, "hello".into(), vec![]);
        let mut guard = state.builder().write_counter().lock().await;
        *guard.counter = 42;
        let guard = guard.relock().read_counter().read_name().lock().await;
        assert_eq!(*guard.counter, 42);
        assert_eq!(&*guard.name, "hello");
    });
}

#[test]
fn lock_all_read() {
    block_on(async {
        let state = MyStateLock::new(1, "all".into(), vec![2]);
        let guard = state.lock_all().await;
        assert_eq!(*guard.counter, 1);
        assert_eq!(&*guard.name, "all");
        assert_eq!(&*guard.data, &[2]);
    });
}

#[test]
fn lock_all_mut() {
    block_on(async {
        let state = MyStateLock::new(0, String::new(), vec![]);
        let mut guard = state.lock_all_mut().await;
        *guard.counter = 10;
        *guard.name = "mutated".into();
        guard.data.push(1);
        assert_eq!(*guard.counter, 10);
    });
}

#[test]
fn try_lock_succeeds_when_unlocked() {
    let state = MyStateLock::new(0, String::new(), vec![]);
    let guard = state.builder().write_counter().read_name().try_lock();
    assert!(guard.is_some());
}

#[test]
fn try_lock_all_succeeds() {
    let state = MyStateLock::new(1, "x".into(), vec![]);
    let guard = state.try_lock_all();
    assert!(guard.is_some());
    let guard = guard.unwrap();
    assert_eq!(*guard.counter, 1);
}

#[test]
fn try_lock_all_mut_succeeds() {
    let state = MyStateLock::new(0, String::new(), vec![]);
    let guard = state.try_lock_all_mut();
    assert!(guard.is_some());
    let mut guard = guard.unwrap();
    *guard.counter = 99;
    assert_eq!(*guard.counter, 99);
}

#[test]
fn per_field_direct_read() {
    block_on(async {
        let state = MyStateLock::new(42, "direct".into(), vec![]);
        let counter = state.read_counter().await;
        assert_eq!(*counter, 42);
    });
}

#[test]
fn per_field_direct_write() {
    block_on(async {
        let state = MyStateLock::new(0, String::new(), vec![]);
        let mut counter = state.write_counter().await;
        *counter = 77;
        assert_eq!(*counter, 77);
    });
}

#[test]
fn per_field_try_read() {
    let state = MyStateLock::new(5, String::new(), vec![]);
    let counter = state.try_read_counter();
    assert!(counter.is_some());
    assert_eq!(*counter.unwrap(), 5);
}

#[test]
fn per_field_try_write() {
    let state = MyStateLock::new(0, String::new(), vec![]);
    let mut counter = state.try_write_counter().unwrap();
    *counter = 88;
    assert_eq!(*counter, 88);
}

#[smart_lock]
struct GenericState<T: Send + Sync + 'static> {
    value: T,
    count: u32,
}

#[test]
fn generic_struct_basic() {
    block_on(async {
        let state = GenericStateLock::new(vec![1, 2, 3], 0);
        let mut guard = state.builder().write_value().write_count().lock().await;
        guard.value.push(4);
        *guard.count += 1;
        assert_eq!(&*guard.value, &[1, 2, 3, 4]);
        assert_eq!(*guard.count, 1);
    });
}

#[test]
fn generic_struct_into_inner() {
    let state = GenericStateLock::new("hello".to_string(), 42);
    let original: GenericState<String> = state.into_inner();
    assert_eq!(original.value, "hello");
    assert_eq!(original.count, 42);
}

#[test]
fn lock_rest_read_smoke() {
    block_on(async {
        let state = MyStateLock::new(0, "rest".into(), vec![1]);
        let mut guard = state.builder().write_counter().lock_rest_read().await;
        *guard.counter = 42;
        assert_eq!(*guard.counter, 42);
        assert_eq!(&*guard.name, "rest");
        assert_eq!(&*guard.data, &[1]);
    });
}

// --- #[no_lock] fields ---

use std::sync::atomic::{AtomicU32, Ordering};

#[smart_lock]
struct WithNoLock {
    counter: u32,
    #[no_lock]
    synced: AtomicU32,
    name: String,
}

#[test]
fn no_lock_lock_all_read() {
    block_on(async {
        let state = WithNoLockLock::new(1, AtomicU32::new(2), "test".into());
        let guard = state.lock_all().await;
        assert_eq!(*guard.counter, 1);
        assert_eq!(guard.synced.load(Ordering::Relaxed), 2);
        assert_eq!(&*guard.name, "test");
    });
}

#[test]
fn no_lock_lock_all_mut() {
    block_on(async {
        let state = WithNoLockLock::new(0, AtomicU32::new(0), String::new());
        let mut guard = state.lock_all_mut().await;
        *guard.counter = 10;
        guard.synced.store(20, Ordering::Relaxed);
        *guard.name = "mutated".into();
        assert_eq!(*guard.counter, 10);
        assert_eq!(guard.synced.load(Ordering::Relaxed), 20);
    });
}

#[test]
fn no_lock_builder_mixed() {
    block_on(async {
        let state = WithNoLockLock::new(0, AtomicU32::new(0), "hello".into());
        let mut guard = state.builder().write_counter().read_name().lock().await;
        *guard.counter = 42;
        guard.synced.store(99, Ordering::Relaxed);
        assert_eq!(*guard.counter, 42);
        assert_eq!(guard.synced.load(Ordering::Relaxed), 99);
        assert_eq!(&*guard.name, "hello");
    });
}

#[test]
fn no_lock_into_inner() {
    let state = WithNoLockLock::new(42, AtomicU32::new(99), "inner".into());
    let original: WithNoLock = state.into_inner();
    assert_eq!(original.counter, 42);
    assert_eq!(original.synced.load(Ordering::Relaxed), 99);
    assert_eq!(original.name, "inner");
}

#[test]
fn no_lock_from_impl() {
    let original = WithNoLock { counter: 10, synced: AtomicU32::new(20), name: "from".into() };
    let state: WithNoLockLock = original.into();
    let back = state.into_inner();
    assert_eq!(back.counter, 10);
    assert_eq!(back.synced.load(Ordering::Relaxed), 20);
}

#[test]
fn no_lock_get_mut() {
    let mut state = WithNoLockLock::new(0, AtomicU32::new(0), String::new());
    *state.get_mut_counter() = 42;
    *state.get_mut_synced() = AtomicU32::new(99);
    assert_eq!(*state.get_mut_counter(), 42);
    assert_eq!(state.get_mut_synced().load(Ordering::Relaxed), 99);
}

#[test]
fn no_lock_try_lock_all() {
    let state = WithNoLockLock::new(1, AtomicU32::new(2), "try".into());
    let guard = state.try_lock_all();
    assert!(guard.is_some());
    let guard = guard.unwrap();
    assert_eq!(*guard.counter, 1);
    assert_eq!(guard.synced.load(Ordering::Relaxed), 2);
}

#[test]
fn no_lock_relock() {
    block_on(async {
        let state = WithNoLockLock::new(0, AtomicU32::new(0), "hello".into());
        let mut guard = state.builder().write_counter().lock().await;
        *guard.counter = 42;
        guard.synced.store(10, Ordering::Relaxed);
        let guard = guard.relock().read_counter().read_name().lock().await;
        assert_eq!(*guard.counter, 42);
        assert_eq!(guard.synced.load(Ordering::Relaxed), 10);
    });
}

#[test]
fn no_lock_lock_rest_read() {
    block_on(async {
        let state = WithNoLockLock::new(0, AtomicU32::new(5), "rest".into());
        let mut guard = state.builder().write_counter().lock_rest_read().await;
        *guard.counter = 42;
        assert_eq!(guard.synced.load(Ordering::Relaxed), 5);
        assert_eq!(&*guard.name, "rest");
    });
}

// --- try_upgrade on guard ---

#[test]
fn guard_try_upgrade_succeeds() {
    block_on(async {
        let state = MyStateLock::new(0, String::new(), vec![]);
        let guard = state.builder().upgrade_counter().lock().await;
        let mut guard = guard.try_upgrade_counter().unwrap();
        *guard.counter = 42;
        assert_eq!(*guard.counter, 42);
    });
}

#[test]
fn guard_try_upgrade_fails_returns_original() {
    block_on(async {
        let state = MyStateLock::new(0, String::new(), vec![]);
        let _reader = state.read_counter().await;
        let guard = state.builder().upgrade_counter().lock().await;
        let guard = guard.try_upgrade_counter().unwrap_err();
        assert_eq!(*guard.counter, 0); // still readable
    });
}

// --- Debug ---

#[test]
fn debug_impl() {
    let state = MyStateLock::new(42, "hello".into(), vec![1]);
    let debug_str = format!("{:?}", state);
    assert!(debug_str.contains("MyStateLock"));
}

// --- Guard Debug ---

#[test]
fn guard_debug_impl() {
    block_on(async {
        let state = MyStateLock::new(0, String::new(), vec![]);
        let guard = state.builder().read_counter().lock().await;
        let debug_str = format!("{:?}", guard);
        assert!(debug_str.contains("MyStateLockGuard"));
    });
}

use smart_lock::smart_lock;

#[smart_lock]
struct MyState {
    counter: u32,
    name: String,
    data: Vec<u8>,
}

#[tokio::test]
async fn create_and_lock_all_fields_mut() {
    let state = MyStateLock::new(0, "hello".into(), vec![]);

    let mut guard = state
        .builder()
        .write_counter()
        .write_name()
        .write_data()
        .lock()
        .await;

    *guard.counter += 1;
    *guard.name = "world".into();
    guard.data.push(42);

    assert_eq!(*guard.counter, 1);
    assert_eq!(*guard.name, "world");
    assert_eq!(*guard.data, vec![42]);
}

#[tokio::test]
async fn lock_all_read() {
    let state = MyStateLock::new(10, "test".into(), vec![1, 2, 3]);

    let guard = state.lock_all().await;

    assert_eq!(*guard.counter, 10);
    assert_eq!(*guard.name, "test");
    assert_eq!(*guard.data, vec![1, 2, 3]);
}

#[tokio::test]
async fn lock_all_mut() {
    let state = MyStateLock::new(0, "start".into(), vec![]);

    let mut guard = state.lock_all_mut().await;

    *guard.counter = 42;
    *guard.name = "changed".into();
    guard.data.push(1);

    assert_eq!(*guard.counter, 42);
    assert_eq!(*guard.name, "changed");
    assert_eq!(*guard.data, vec![1]);
}

#[tokio::test]
async fn mutation_persists_across_locks() {
    let state = MyStateLock::new(0, "start".into(), vec![]);

    {
        let mut guard = state.builder().write_counter().lock().await;
        *guard.counter = 42;
    }

    {
        let guard = state.builder().read_counter().lock().await;
        assert_eq!(*guard.counter, 42);
    }
}

#[tokio::test]
async fn mixed_read_and_write() {
    let state = MyStateLock::new(5, "mixed".into(), vec![10]);

    let mut guard = state
        .builder()
        .write_counter()
        .read_name()
        .read_data()
        .lock()
        .await;

    // counter is write-locked
    *guard.counter += 10;
    assert_eq!(*guard.counter, 15);

    // name and data are read-locked
    assert_eq!(*guard.name, "mixed");
    assert_eq!(*guard.data, vec![10]);
}

#[tokio::test]
async fn from_impl() {
    let original = MyState {
        counter: 99,
        name: "from".into(),
        data: vec![1, 2],
    };

    let state: MyStateLock = original.into();
    let guard = state.lock_all().await;

    assert_eq!(*guard.counter, 99);
    assert_eq!(*guard.name, "from");
    assert_eq!(*guard.data, vec![1, 2]);
}

#[tokio::test]
async fn per_field_direct_read() {
    let state = MyStateLock::new(10, "hello".into(), vec![1, 2]);

    let counter = state.read_counter().await;
    assert_eq!(*counter, 10);

    let name = state.read_name().await;
    assert_eq!(*name, "hello");
}

#[tokio::test]
async fn per_field_direct_write() {
    let state = MyStateLock::new(0, "".into(), vec![]);

    {
        let mut counter = state.write_counter().await;
        *counter = 99;
    }

    let counter = state.read_counter().await;
    assert_eq!(*counter, 99);
}

#[tokio::test]
async fn per_field_try_read() {
    let state = MyStateLock::new(42, "test".into(), vec![]);

    let counter = state.try_read_counter();
    assert!(counter.is_some());
    assert_eq!(*counter.unwrap(), 42);
}

#[tokio::test]
async fn per_field_try_write() {
    let state = MyStateLock::new(0, "".into(), vec![]);

    if let Some(mut counter) = state.try_write_counter() {
        *counter = 7;
    }

    let counter = state.read_counter().await;
    assert_eq!(*counter, 7);
}

#[tokio::test]
async fn builder_selective_fields() {
    let state = MyStateLock::new(0, "hello".into(), vec![]);

    // Only lock counter for writing, leave others unlocked
    let mut guard = state.builder().write_counter().lock().await;
    *guard.counter = 100;
    assert_eq!(*guard.counter, 100);
    // guard.name and guard.data are Unlocked — no Deref available (compile error if tried)
}

#[tokio::test]
async fn upgrade_read_to_write() {
    let state = MyStateLock::new(0, "hello".into(), vec![]);

    // Start with upgradable read on counter
    let guard = state.builder().upgrade_counter().lock().await;
    assert_eq!(*guard.counter, 0); // can read

    // Upgrade to write
    let mut guard = guard.upgrade_counter().await;
    *guard.counter = 42; // can now write
    assert_eq!(*guard.counter, 42);
}

#[tokio::test]
async fn upgrade_persists() {
    let state = MyStateLock::new(10, "test".into(), vec![]);

    {
        let guard = state.builder().upgrade_counter().lock().await;
        assert_eq!(*guard.counter, 10);
        let mut guard = guard.upgrade_counter().await;
        *guard.counter = 99;
    }

    let guard = state.builder().read_counter().lock().await;
    assert_eq!(*guard.counter, 99);
}

#[tokio::test]
async fn downgrade_write_to_read() {
    let state = MyStateLock::new(0, "hello".into(), vec![]);

    let mut guard = state.builder().write_counter().read_name().lock().await;
    *guard.counter = 42;

    // Downgrade counter from write to read
    let guard = guard.downgrade_counter();
    assert_eq!(*guard.counter, 42); // can still read
    assert_eq!(*guard.name, "hello"); // name unchanged
    // *guard.counter = 0; // would be compile error — now ReadLocked
}

#[tokio::test]
async fn downgrade_upgrade_to_read() {
    let state = MyStateLock::new(5, "test".into(), vec![]);

    let guard = state.builder().upgrade_counter().lock().await;
    assert_eq!(*guard.counter, 5);

    // Downgrade upgradable to regular read (releases upgrade slot)
    let guard = guard.downgrade_counter();
    assert_eq!(*guard.counter, 5);
}

#[tokio::test]
async fn upgrade_with_other_fields() {
    let state = MyStateLock::new(0, "hello".into(), vec![1, 2]);

    // Upgrade counter, read name, write data
    let mut guard = state
        .builder()
        .upgrade_counter()
        .read_name()
        .write_data()
        .lock()
        .await;

    assert_eq!(*guard.counter, 0);
    assert_eq!(*guard.name, "hello");
    guard.data.push(3);

    // Upgrade counter to write
    let mut guard = guard.upgrade_counter().await;
    *guard.counter = 100;
    assert_eq!(*guard.counter, 100);
    assert_eq!(*guard.name, "hello");
    assert_eq!(*guard.data, vec![1, 2, 3]);
}

#[tokio::test]
async fn per_field_upgrade_accessor() {
    let state = MyStateLock::new(0, "test".into(), vec![]);

    {
        let guard = state.upgrade_counter().await;
        assert_eq!(*guard, 0);
    }

    let guard = state.try_upgrade_counter();
    assert!(guard.is_some());
}

// --- into_inner ---

#[tokio::test]
async fn into_inner_reconstructs_original() {
    let state = MyStateLock::new(42, "hello".into(), vec![1, 2, 3]);
    let original = state.into_inner();

    assert_eq!(original.counter, 42);
    assert_eq!(original.name, "hello");
    assert_eq!(original.data, vec![1, 2, 3]);
}

#[tokio::test]
async fn into_inner_after_mutation() {
    let state = MyStateLock::new(0, "start".into(), vec![]);

    {
        let mut guard = state.builder().write_counter().write_name().lock().await;
        *guard.counter = 99;
        *guard.name = "changed".into();
    }

    let original = state.into_inner();
    assert_eq!(original.counter, 99);
    assert_eq!(original.name, "changed");
}

// --- get_mut ---

#[tokio::test]
async fn get_mut_per_field() {
    let mut state = MyStateLock::new(0, "hello".into(), vec![]);

    *state.get_mut_counter() = 42;
    state.get_mut_data().push(1);

    let guard = state.lock_all().await;
    assert_eq!(*guard.counter, 42);
    assert_eq!(*guard.data, vec![1]);
}

// --- relock ---

#[tokio::test]
async fn relock_drops_and_rebuilds() {
    let state = MyStateLock::new(0, "hello".into(), vec![]);

    // Lock counter for writing
    let mut guard = state.builder().write_counter().lock().await;
    *guard.counter = 42;

    // Relock: drops the current guard, returns a new builder
    let guard = guard.relock().read_counter().read_name().lock().await;
    assert_eq!(*guard.counter, 42);
    assert_eq!(*guard.name, "hello");
}

#[tokio::test]
async fn relock_allows_different_fields() {
    let state = MyStateLock::new(0, "hello".into(), vec![1]);

    let mut guard = state.builder().write_counter().lock().await;
    *guard.counter = 10;

    // Relock and grab different fields
    let mut guard = guard.relock().write_name().write_data().lock().await;
    *guard.name = "world".into();
    guard.data.push(2);

    drop(guard);

    let guard = state.lock_all().await;
    assert_eq!(*guard.counter, 10);
    assert_eq!(*guard.name, "world");
    assert_eq!(*guard.data, vec![1, 2]);
}

// --- Generic structs ---

#[smart_lock]
struct GenericState<T: Clone + Send + Sync + 'static> {
    value: T,
    count: u32,
}

#[tokio::test]
async fn generic_struct_basic() {
    let state = GenericStateLock::new("hello".to_string(), 0);

    let mut guard = state.builder().write_value().write_count().lock().await;
    *guard.value = "world".to_string();
    *guard.count = 1;

    assert_eq!(*guard.value, "world");
    assert_eq!(*guard.count, 1);
}

#[tokio::test]
async fn generic_struct_into_inner() {
    let state = GenericStateLock::new(vec![1, 2, 3], 42);
    let original = state.into_inner();
    assert_eq!(original.value, vec![1, 2, 3]);
    assert_eq!(original.count, 42);
}

#[tokio::test]
async fn generic_struct_from() {
    let original = GenericState {
        value: 99i64,
        count: 5,
    };
    let state: GenericStateLock<i64> = original.into();
    let guard = state.lock_all().await;
    assert_eq!(*guard.value, 99);
    assert_eq!(*guard.count, 5);
}

#[tokio::test]
async fn generic_struct_relock() {
    let state = GenericStateLock::new("hello".to_string(), 0);

    let mut guard = state.builder().write_count().lock().await;
    *guard.count = 5;

    let guard = guard.relock().read_value().read_count().lock().await;
    assert_eq!(*guard.value, "hello");
    assert_eq!(*guard.count, 5);
}

// --- Attribute passthrough (doc comments) ---

#[smart_lock]
struct DocStruct {
    /// This is a documented field
    x: u32,
    y: u32,
}

#[tokio::test]
async fn doc_struct_compiles_with_attrs() {
    let state = DocStructLock::new(1, 2);
    let guard = state.lock_all().await;
    assert_eq!(*guard.x, 1);
    assert_eq!(*guard.y, 2);
}

// --- try_lock ---

#[tokio::test]
async fn try_lock_succeeds_when_unlocked() {
    let state = MyStateLock::new(42, "hello".into(), vec![1]);

    let guard = state.builder().write_counter().read_name().try_lock();
    assert!(guard.is_some());
    let guard = guard.unwrap();
    assert_eq!(*guard.counter, 42);
    assert_eq!(*guard.name, "hello");
}

#[tokio::test]
async fn try_lock_fails_when_write_held() {
    let state = MyStateLock::new(0, "".into(), vec![]);

    // Hold a write lock on counter
    let _hold = state.write_counter().await;

    // try_lock requesting write on counter should fail
    let guard = state.builder().write_counter().try_lock();
    assert!(guard.is_none());
}

#[tokio::test]
async fn try_lock_fails_when_read_blocked_by_write() {
    let state = MyStateLock::new(0, "".into(), vec![]);

    let _hold = state.write_counter().await;

    // try_lock requesting read on counter should also fail (writer holds it)
    let guard = state.builder().read_counter().try_lock();
    assert!(guard.is_none());
}

#[tokio::test]
async fn try_lock_succeeds_on_different_fields() {
    let state = MyStateLock::new(0, "held".into(), vec![]);

    // Hold write on name
    let _hold = state.write_name().await;

    // try_lock on counter should succeed (different field)
    let guard = state.builder().write_counter().try_lock();
    assert!(guard.is_some());
}

#[tokio::test]
async fn try_lock_releases_on_partial_failure() {
    let state = MyStateLock::new(0, "".into(), vec![]);

    // Hold write on name (field index 1)
    let _hold = state.write_name().await;

    // try_lock requesting counter (succeeds) then name (fails)
    // counter lock should be released on failure
    let guard = state.builder().write_counter().write_name().try_lock();
    assert!(guard.is_none());

    // Verify counter is NOT still locked (the partial acquisition was dropped)
    let counter = state.try_write_counter();
    assert!(counter.is_some());
}

#[tokio::test]
async fn try_lock_all_unlocked_fields_returns_some() {
    let state = MyStateLock::new(10, "test".into(), vec![]);

    // try_lock with no fields requested should succeed
    let guard = state.builder().try_lock();
    assert!(guard.is_some());
}

// --- try_lock_all / try_lock_all_mut ---

#[tokio::test]
async fn try_lock_all_succeeds() {
    let state = MyStateLock::new(42, "hello".into(), vec![1]);

    let guard = state.try_lock_all();
    assert!(guard.is_some());
    let guard = guard.unwrap();
    assert_eq!(*guard.counter, 42);
    assert_eq!(*guard.name, "hello");
    assert_eq!(*guard.data, vec![1]);
}

#[tokio::test]
async fn try_lock_all_fails_when_write_held() {
    let state = MyStateLock::new(0, "".into(), vec![]);

    let _hold = state.write_counter().await;
    assert!(state.try_lock_all().is_none());
}

#[tokio::test]
async fn try_lock_all_mut_succeeds() {
    let state = MyStateLock::new(0, "".into(), vec![]);

    let guard = state.try_lock_all_mut();
    assert!(guard.is_some());
    let mut guard = guard.unwrap();
    *guard.counter = 99;
    assert_eq!(*guard.counter, 99);
}

#[tokio::test]
async fn try_lock_all_mut_fails_when_read_held() {
    let state = MyStateLock::new(0, "".into(), vec![]);

    let _hold = state.read_counter().await;
    assert!(state.try_lock_all_mut().is_none());
}

// --- lock_rest_read ---

#[tokio::test]
async fn lock_rest_read_fills_unlocked_with_read() {
    let state = MyStateLock::new(10, "hello".into(), vec![1, 2]);

    // Only write counter, rest should become ReadLocked
    let mut guard = state.builder().write_counter().lock_rest_read().await;
    *guard.counter += 1;
    assert_eq!(*guard.counter, 11);
    assert_eq!(*guard.name, "hello");
    assert_eq!(*guard.data, vec![1, 2]);
}

#[tokio::test]
async fn lock_rest_read_preserves_explicit_modes() {
    let state = MyStateLock::new(0, "test".into(), vec![]);

    // Write counter, upgrade name, rest (data) becomes ReadLocked
    let guard = state
        .builder()
        .write_counter()
        .upgrade_name()
        .lock_rest_read()
        .await;

    assert_eq!(*guard.counter, 0);
    assert_eq!(*guard.name, "test");
    assert_eq!(*guard.data, vec![]);
}

#[tokio::test]
async fn try_lock_rest_read_succeeds() {
    let state = MyStateLock::new(42, "try".into(), vec![9]);

    let guard = state.builder().write_counter().try_lock_rest_read();
    assert!(guard.is_some());
    let mut guard = guard.unwrap();
    *guard.counter = 100;
    assert_eq!(*guard.counter, 100);
    assert_eq!(*guard.name, "try");
    assert_eq!(*guard.data, vec![9]);
}

#[tokio::test]
async fn try_lock_rest_read_fails_on_contention() {
    let state = MyStateLock::new(0, "held".into(), vec![]);

    // Hold write on name
    let _hold = state.write_name().await;

    // try_lock_rest_read needs read on name, should fail
    let guard = state.builder().write_counter().try_lock_rest_read();
    assert!(guard.is_none());
}

#[tokio::test]
async fn lock_rest_read_all_unlocked() {
    let state = MyStateLock::new(5, "all".into(), vec![1]);

    // No explicit modes — everything becomes ReadLocked
    let guard = state.builder().lock_rest_read().await;
    assert_eq!(*guard.counter, 5);
    assert_eq!(*guard.name, "all");
    assert_eq!(*guard.data, vec![1]);
}

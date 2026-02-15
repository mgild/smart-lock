use smart_lock::smart_lock;

#[smart_lock]
struct Fields10 {
    f0: u64,
    f1: u64,
    f2: u64,
    f3: u64,
    f4: u64,
    f5: u64,
    f6: u64,
    f7: u64,
    f8: u64,
    f9: u64,
}

#[tokio::test]
async fn fields_10_compiles() {
    let state = Fields10Lock::new(0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    let guard = state.lock_all().await;
    assert_eq!(*guard.f0, 0);
}

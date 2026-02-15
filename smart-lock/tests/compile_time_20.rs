use smart_lock::smart_lock;

#[smart_lock]
struct Fields20 {
    f0: u64, f1: u64, f2: u64, f3: u64, f4: u64,
    f5: u64, f6: u64, f7: u64, f8: u64, f9: u64,
    f10: u64, f11: u64, f12: u64, f13: u64, f14: u64,
    f15: u64, f16: u64, f17: u64, f18: u64, f19: u64,
}

#[tokio::test]
async fn fields_20_compiles() {
    let state = Fields20Lock::new(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0);
    let guard = state.lock_all().await;
    assert_eq!(*guard.f0, 0);
}

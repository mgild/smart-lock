use smart_lock::smart_lock;

#[smart_lock]
struct Fields40 {
    f0: u64, f1: u64, f2: u64, f3: u64, f4: u64,
    f5: u64, f6: u64, f7: u64, f8: u64, f9: u64,
    f10: u64, f11: u64, f12: u64, f13: u64, f14: u64,
    f15: u64, f16: u64, f17: u64, f18: u64, f19: u64,
    f20: u64, f21: u64, f22: u64, f23: u64, f24: u64,
    f25: u64, f26: u64, f27: u64, f28: u64, f29: u64,
    f30: u64, f31: u64, f32: u64, f33: u64, f34: u64,
    f35: u64, f36: u64, f37: u64, f38: u64, f39: u64,
}

#[tokio::test]
async fn fields_40_compiles() {
    let state = Fields40Lock::new(
        0,0,0,0,0,0,0,0,0,0,
        0,0,0,0,0,0,0,0,0,0,
        0,0,0,0,0,0,0,0,0,0,
        0,0,0,0,0,0,0,0,0,0,
    );
    let guard = state.lock_all().await;
    assert_eq!(*guard.f0, 0);
}

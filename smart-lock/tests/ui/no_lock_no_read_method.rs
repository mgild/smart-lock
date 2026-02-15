use smart_lock::smart_lock;
use std::sync::atomic::AtomicU32;

#[smart_lock]
struct Foo {
    x: u32,
    #[no_lock]
    synced: AtomicU32,
}

#[tokio::main]
async fn main() {
    let state = FooLock::new(0, AtomicU32::new(0));
    let guard = state.builder().read_synced().lock().await;
    // ERROR: read_synced not found — #[no_lock] fields have no builder methods
}

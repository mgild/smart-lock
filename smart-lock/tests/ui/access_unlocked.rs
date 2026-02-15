use smart_lock::smart_lock;

#[smart_lock]
struct Foo {
    x: u32,
    y: u32,
}

#[tokio::main]
async fn main() {
    let state = FooLock::new(0, 0);
    let guard = state.builder().read_x().lock().await;
    let _ = *guard.y; // ERROR: Unlocked has no Deref
}

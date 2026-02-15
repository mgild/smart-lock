use smart_lock::smart_lock;

#[smart_lock]
struct Foo {
    x: u32,
}

#[tokio::main]
async fn main() {
    let state = FooLock::new(0);
    let guard = state.builder().read_x().write_x().lock().await;
    // ERROR: write_x not available when x is already ReadLocked
}

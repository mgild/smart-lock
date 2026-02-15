use smart_lock::smart_lock;

#[smart_lock]
struct Foo {
    x: u32,
}

#[tokio::main]
async fn main() {
    let state = FooLock::new(0);
    let mut guard = state.builder().read_x().lock().await;
    *guard.x = 1; // ERROR: ReadLocked has no DerefMut
}

//! Minimal smart-lock example.
//!
//! Run with: `cargo run --example basic`

use smart_lock::smart_lock;

#[smart_lock]
struct Counter {
    value: u32,
    label: String,
}

#[tokio::main]
async fn main() {
    let state = CounterLock::new(0, "hits".into());

    // Builder: select fields and lock modes
    let mut guard = state.builder().write_value().read_label().lock().await;
    *guard.value += 1;
    println!("{}: {}", *guard.label, *guard.value);

    drop(guard);

    // Convenience: lock all fields for reading
    let guard = state.lock_all().await;
    println!("Final: {} = {}", *guard.label, *guard.value);
}

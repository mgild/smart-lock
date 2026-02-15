//! Demonstrates upgradable locks: read first, upgrade to write only when needed.
//!
//! This avoids holding an exclusive write lock for the entire operation when
//! the write may not be necessary.
//!
//! Run with: `cargo run --example upgradable`

use smart_lock::smart_lock;

#[smart_lock]
struct Inventory {
    items: Vec<String>,
    last_modified: String,
}

#[tokio::main]
async fn main() {
    let state = InventoryLock::new(vec!["apple".into(), "banana".into()], "init".into());

    // Scenario: add an item only if it doesn't already exist.
    //
    // Start with an upgradable read — allows reading without blocking other
    // readers, and can be atomically promoted to a write lock when needed.
    let item = "cherry";

    let guard = state
        .builder()
        .upgrade_items()
        .upgrade_last_modified()
        .lock()
        .await;

    if guard.items.contains(&item.to_string()) {
        println!("'{}' already exists, no write needed", item);
        // Guard drops here — no write lock was ever acquired
    } else {
        println!("'{}' not found, upgrading to write...", item);

        // Atomic upgrade: waits for other readers to drain, then grants
        // exclusive access. No gap where the lock is released.
        let mut guard = guard.upgrade_items().await;
        guard.items.push(item.into());

        let mut guard = guard.upgrade_last_modified().await;
        *guard.last_modified = "added cherry".into();

        println!("Added '{}', items: {:?}", item, &*guard.items);
    }

    // Demonstrate downgrade: write first, then downgrade to read
    let mut guard = state.builder().write_items().lock().await;
    guard.items.push("date".into());

    // Downgrade to read — atomic, synchronous, immediately unblocks other readers
    let guard = guard.downgrade_items();
    println!("After downgrade, items: {:?}", &*guard.items);
}

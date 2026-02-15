use smart_lock::smart_lock;
use std::sync::atomic::AtomicU32;

#[smart_lock]
struct Bad {
    #[no_lock]
    #[no_lock]
    synced: AtomicU32,
}

fn main() {}

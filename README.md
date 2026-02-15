# smart-lock

Per-field async `RwLock` with **compile-time access control** via proc macro.

Annotate a struct with `#[smart_lock]` and get a type-safe builder that lets you select exactly which fields to lock and how (read, write, or upgradable). Unlocked fields produce **compile errors** on access, not runtime panics. Deadlock-free by construction.

**Runtime-agnostic** — built on [`async-lock`](https://docs.rs/async-lock), works with tokio, async-std, smol, or any async runtime.

## Quick Start

```rust
use smart_lock::smart_lock;

#[smart_lock]
struct MyState {
    counter: u32,
    name: String,
    data: Vec<u8>,
}

#[tokio::main]
async fn main() {
    let state = MyStateLock::new(0, "hello".into(), vec![]);

    let mut guard = state
        .builder()
        .write_counter()
        .read_name()
        .lock()
        .await;

    *guard.counter += 1;        // write access
    println!("{}", guard.name); // read access
    // guard.data — compile error: field not locked
}
```

## What It Generates

For a struct `Foo`, `#[smart_lock]` generates:

| Type | Purpose |
|------|---------|
| `FooLock` | Wrapper holding an `RwLock<T>` per field |
| `FooLockBuilder` | Type-state builder for selecting lock modes |
| `FooLockGuard` | Guard with per-field access encoded in the type system |
| `From<Foo> for FooLock` | Conversion from the original struct |

## Three Ways to Lock

### 1. Builder (multi-field, deadlock-free)

Select exactly which fields you need and how:

```rust
let mut guard = state
    .builder()
    .write_x()      // exclusive write access
    .read_y()       // shared read access
    .upgrade_z()    // read now, upgrade to write later
    .lock()
    .await;

*guard.x += 1;
println!("{}", guard.y);
let mut guard = guard.upgrade_z().await;  // atomic upgrade
*guard.z = 42;
```

#### `lock_rest_read` — write a few, read the rest

For large structs, listing every field is verbose. `lock_rest_read()` fills any unlocked fields with read locks:

```rust
let mut guard = state
    .builder()
    .write_counter()        // explicit write
    .lock_rest_read()       // name, data → ReadLocked
    .await;

*guard.counter += 1;
println!("{} {:?}", guard.name, guard.data);
```

Non-blocking variant: `try_lock_rest_read()`.

### 2. Direct per-field accessors

Quick single-field access without the builder:

```rust
let x = state.read_x().await;        // RwLockReadGuard
let mut y = state.write_y().await;    // RwLockWriteGuard
*y += 1;

// Non-blocking variants
if let Some(x) = state.try_read_x() {
    println!("{}", *x);
}
```

### 3. Non-blocking multi-field lock

Try to acquire all requested locks without blocking. Returns `None` if any lock is held:

```rust
if let Some(mut guard) = state.builder().write_x().read_y().try_lock() {
    *guard.x += 1;
    println!("{}", guard.y);
}
```

On failure, any partially-acquired locks are automatically released.

### 4. Lock all fields at once

```rust
let guard = state.lock_all().await;       // read all
let mut guard = state.lock_all_mut().await; // write all
```

## Compile-Time Safety

The type-state builder encodes each field's lock mode as a generic parameter. This gives three guarantees at compile time — no runtime panics, no `unwrap()`, no "oops I forgot to lock it":

**1. Unlocked fields cannot be accessed:**

```rust
let guard = state.builder().write_x().lock().await;
let _ = *guard.y;
// ERROR: FieldGuard<'_, u32, Unlocked> doesn't implement Deref
```

**2. Read-locked fields cannot be mutated:**

```rust
let mut guard = state.builder().read_x().lock().await;
*guard.x = 10;
// ERROR: FieldGuard<'_, u32, ReadLocked> doesn't implement DerefMut
```

**3. Fields cannot be double-locked:**

```rust
state.builder()
    .write_x()
    .read_x()  // ERROR: method not found — write_x consumed the Unlocked state
```

## Upgradable Locks

Acquire a field as upgradable read, then atomically upgrade to write — no gap where the lock is released:

```rust
let guard = state
    .builder()
    .upgrade_counter()
    .read_name()
    .lock()
    .await;

let val = *guard.counter;  // read access

// Atomically upgrade to write (waits for other readers to drain)
let mut guard = guard.upgrade_counter().await;
*guard.counter = val + 1;  // write access
```

Only one upgradable reader per field at a time, preventing the classic two-upgraders deadlock.

> **Warning:** While `upgrade_field().await` waits for readers to drain, the guard continues holding all other locks. If another task holds a read lock on that field and is waiting to upgrade a different field that *this* guard holds, both tasks will deadlock. To upgrade multiple fields safely, either acquire them as `write_*()` upfront or use `.relock()` to drop all locks and re-acquire with the desired modes.

### Downgrade

Write or upgradable locks can be atomically downgraded to read locks:

```rust
let mut guard = state.builder().write_counter().lock().await;
*guard.counter = 42;

let guard = guard.downgrade_counter();  // atomic, sync (no .await)
println!("{}", *guard.counter);         // still readable, other readers unblocked
```

| Transition | Method | Async? |
|-----------|--------|--------|
| Upgrade &rarr; Write | `.upgrade_field().await` | yes (waits for readers) |
| Write &rarr; Read | `.downgrade_field()` | no (atomic) |
| Upgrade &rarr; Read | `.downgrade_field()` | no (atomic) |

## Relock

Drop the current guard and immediately get a fresh builder for the same lock. Useful for changing which fields you hold without dropping and re-borrowing the lock:

```rust
let mut guard = state.builder().write_counter().lock().await;
*guard.counter = 42;

// Drop locks, get a new builder for the same state
let guard = guard.relock()
    .read_counter()
    .read_name()
    .lock()
    .await;

assert_eq!(*guard.counter, 42);
```

## Self-synchronized Fields (`#[no_lock]`)

Fields that are already internally synchronized (e.g., `AtomicU32`, `Mutex<T>`, `DashMap`) don't need `RwLock` wrapping. Mark them with `#[no_lock]` to store them as bare `T` and expose them as `&T` on the guard — always accessible, no lock mode needed:

```rust
use std::sync::atomic::{AtomicU32, Ordering};

#[smart_lock]
struct MyState {
    counter: u32,
    #[no_lock]
    request_count: AtomicU32,
    name: String,
}

#[tokio::main]
async fn main() {
    let state = MyStateLock::new(0, AtomicU32::new(0), "hello".into());

    let mut guard = state.builder().write_counter().read_name().lock().await;
    *guard.counter += 1;

    // request_count is always accessible — no lock mode needed
    guard.request_count.fetch_add(1, Ordering::Relaxed);
}
```

`#[no_lock]` fields:
- Are **stored** as bare `T` in the lock struct (not `RwLock<T>`)
- Are **exposed** as `&T` on the guard (always accessible, regardless of which fields are locked)
- Have **no builder methods** (`read_*`/`write_*`/`upgrade_*` are not generated)
- Are **skipped** in `lock_all()`/`lock_all_mut()` lock acquisition (no locking overhead)
- Work with `into_inner()`, `From`, and `get_mut_*`

## Deadlock Prevention

The builder acquires locks in **field declaration order**, regardless of the order you call the builder methods. This prevents ABBA deadlocks:

```rust
// Task 1: requests y then x
let g1 = state.builder().write_y().write_x().lock().await;

// Task 2: requests x then y
let g2 = state.builder().write_x().write_y().lock().await;

// Both acquire in declaration order (x, then y) — no deadlock
```

## Generic Structs

Works with generic type parameters, lifetime parameters, and where clauses:

```rust
#[smart_lock]
struct Cache<K: Eq + Hash + Send + Sync + 'static, V: Send + Sync + 'static> {
    map: HashMap<K, V>,
    hits: u64,
    misses: u64,
}

let cache = CacheLock::new(HashMap::new(), 0, 0);
```

## Additional APIs

### `into_inner` — unwrap the lock

Consume the lock and get the original struct back (no async, no locking needed):

```rust
let state = MyStateLock::new(42, "hello".into(), vec![]);
let original: MyState = state.into_inner();
assert_eq!(original.counter, 42);
```

### `get_mut_*` — exclusive reference bypass

When you have `&mut Lock`, you can access fields without locking (guaranteed no other references exist):

```rust
let mut state = MyStateLock::new(0, "hello".into(), vec![]);
*state.get_mut_counter() = 42;  // no lock needed
```

### `From<OriginalStruct>`

Convert from the original struct:

```rust
let original = MyState { counter: 99, name: "from".into(), data: vec![1, 2] };
let state: MyStateLock = original.into();
```

## Benchmarks

Three scenarios comparing: single `RwLock<Struct>`, manual per-field `RwLock`, and smart-lock. All use `async_lock::RwLock` for a fair comparison. 4 fields, 8 tasks, 1000 ops each.

| Scenario | Single `RwLock<S>` | Manual per-field | smart-lock |
|----------|-------------------|-----------------|------------|
| write_contention (4 writers, different fields) | 217 us | 205 us | 219 us |
| read_heavy (8 tasks, 90% read) | 204 us | 327 us | 374 us |
| mixed_access (writers A, readers B) | 468 us | 322 us | 380 us |

**Takeaways:**

- **Write contention**: All three are comparable (~210-220 us). With `async_lock`'s lightweight RwLock, the single-lock approach doesn't serialize as badly as with tokio's heavier lock. smart-lock matches manual per-field performance.

- **Read heavy**: Single RwLock wins because it's one lock acquisition vs N. When most operations are reads across multiple fields, coarse-grained locking has less overhead.

- **Mixed access**: smart-lock is **1.2x faster** than single RwLock (380 us vs 468 us) when writers on field A don't need to block readers on field B. The ~18% gap vs manual (322 us) is the cost of the FieldGuard abstraction.

Run benchmarks yourself:

```bash
cargo bench
```

## Compile-Time Cost

The proc macro generates per-field type-state machinery (generic parameters, impl blocks, trait bounds). Incremental compile times scale linearly with field count:

| Fields | Incremental compile |
|--------|-------------------|
| 3      | ~0.4s             |
| 10     | ~0.3s             |
| 20     | ~0.4s             |
| 40     | ~0.6s             |

Measured on Apple Silicon with `cargo test --test <file>` (incremental, debug). The 3-field case includes multiple structs in one file. Growth is modest — 40 fields adds roughly 0.2s over the baseline.

## When to Use smart-lock

**Good fit:**
- Structs with heterogeneous field types
- Known, fixed field count
- Multiple concurrent tasks accessing different fields
- You want compile-time proof that field access is correct

See [`examples/session_store.rs`](smart-lock/examples/session_store.rs) for a real-world concurrent session store demonstrating independent field access.

**Not a good fit:**
- Collections (HashMap, Vec, etc.) — use [`DashMap`](https://docs.rs/dashmap)/[`DashSet`](https://docs.rs/dashset) instead (shard-level locking, much less memory overhead)
- Single-field access patterns — just use `RwLock<T>` directly
- Read-heavy workloads accessing all fields — single `RwLock<Struct>` is faster

## Limitations

- Named fields only (no tuple structs or unit structs)
- Field access through `Deref`/`DerefMut` (use `*guard.field` syntax)
- `into_inner()` consumes `self` — when behind `Arc`, unwrap first: `Arc::try_unwrap(arc).unwrap().into_inner()`

## Minimum Supported Rust Version

Rust **1.78** or later (requires `#[diagnostic::on_unimplemented]`).

## License

MIT OR Apache-2.0

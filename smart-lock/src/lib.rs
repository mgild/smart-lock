//! Per-field async `RwLock` with **compile-time access control** via proc macro.
//!
//! Annotate a struct with `#[smart_lock]` and get a type-safe builder that lets
//! you select exactly which fields to lock and how (read, write, or upgradable).
//! Unlocked fields produce **compile errors** on access, not runtime panics.
//! Deadlock-free by construction — locks are always acquired in field declaration order.
//!
//! **Runtime-agnostic** — built on [`async-lock`], works with tokio, async-std, smol,
//! or any async runtime.
//!
//! # Quick Start
//!
//! ```rust
//! use smart_lock::smart_lock;
//!
//! #[smart_lock]
//! struct MyState {
//!     counter: u32,
//!     name: String,
//! }
//!
//! # tokio_test::block_on(async {
//! let state = MyStateLock::new(0, "hello".into());
//!
//! // Builder: select fields and lock modes
//! let mut guard = state.builder().write_counter().read_name().lock().await;
//! *guard.counter += 1;
//! assert_eq!(&*guard.name, "hello");
//! // guard.name = ... — compile error: read-locked field
//! # });
//! ```
//!
//! # Builder (multi-field, deadlock-free)
//!
//! ```rust
//! # use smart_lock::smart_lock;
//! # #[smart_lock]
//! # struct MyState { x: u32, y: u32, z: u32 }
//! # tokio_test::block_on(async {
//! # let state = MyStateLock::new(0, 0, 0);
//! let mut guard = state.builder()
//!     .write_x()
//!     .read_y()
//!     .upgrade_z()
//!     .lock()
//!     .await;
//!
//! *guard.x += 1;
//! let _ = *guard.y;
//!
//! // Atomic upgrade from read to write
//! let mut guard = guard.upgrade_z().await;
//! *guard.z = 42;
//! # });
//! ```
//!
//! # Non-blocking `try_lock`
//!
//! Returns `None` if any requested lock is held, automatically releasing
//! any partially-acquired locks:
//!
//! ```rust
//! # use smart_lock::smart_lock;
//! # #[smart_lock]
//! # struct MyState { x: u32, y: u32 }
//! # let state = MyStateLock::new(0, 0);
//! if let Some(mut guard) = state.builder().write_x().read_y().try_lock() {
//!     *guard.x += 1;
//! }
//!
//! // Or lock all fields at once
//! # tokio_test::block_on(async {
//! {
//!     let guard = state.lock_all().await;
//!     let _ = *guard.x;
//! }
//! let mut guard = state.lock_all_mut().await;
//! *guard.x += 1;
//! # });
//! ```
//!
//! # Direct per-field accessors
//!
//! ```rust
//! # use smart_lock::smart_lock;
//! # #[smart_lock]
//! # struct MyState { x: u32 }
//! # tokio_test::block_on(async {
//! # let state = MyStateLock::new(0);
//! {
//!     let x = state.read_x().await;
//!     assert_eq!(*x, 0);
//! } // read lock dropped
//! let mut x = state.write_x().await;
//! *x += 1;
//! # });
//! ```
//!
//! # Generated Types
//!
//! For a struct `Foo`, `#[smart_lock]` generates:
//!
//! | Type | Purpose |
//! |------|---------|
//! | `FooLock` | Wrapper holding an `RwLock<T>` per field |
//! | `FooLockBuilder` | Type-state builder for selecting lock modes |
//! | `FooLockGuard` | Guard with per-field access encoded in the type system |

mod guard;
mod mode;

pub use guard::FieldGuard;
pub use mode::{LockMode, LockModeKind, Readable, ReadLocked, Unlocked, UpgradeLocked, Writable, WriteLocked};
pub use smart_lock_derive::smart_lock;
pub use async_lock::{RwLock, RwLockReadGuard, RwLockUpgradableReadGuard, RwLockWriteGuard};

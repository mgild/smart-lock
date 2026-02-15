# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-05-15

Initial release.

### Added

- `#[smart_lock]` proc macro: annotate a struct to get per-field async `RwLock` with compile-time access control
- **Type-state builder**: `.builder().write_x().read_y().lock().await` — unlocked fields are compile errors, not runtime panics
- **Deadlock prevention**: locks acquired in field declaration order regardless of builder call order
- **Upgradable locks**: `.upgrade_field().await` for atomic read-to-write upgrade, `.downgrade_field()` for write-to-read
- **`try_upgrade_*()` on guard**: non-blocking upgrade attempt, returns `Result` with original guard on failure
- **Direct per-field accessors**: `.read_x().await`, `.write_x().await`, `.try_read_x()`, `.try_write_x()`
- **Non-blocking multi-field lock**: `.try_lock()` on builder, `try_lock_all()`, `try_lock_all_mut()`
- **`lock_rest_read()`**: fill unlocked fields with read locks, preserving explicit modes
- **Relock**: drop current guard and get a fresh builder for the same lock
- **`#[no_lock]`**: skip `RwLock` wrapping for self-synchronized fields (e.g., `AtomicU32`, `Mutex<T>`)
- **`into_inner()`**: consume the lock and recover the original struct
- **`get_mut_*()`**: bypass locking with `&mut self` access
- **`From<OriginalStruct>`**: conversion from the original struct
- **Generic struct support**: works with type parameters, lifetimes, and where clauses
- **Attribute passthrough**: doc comments and other attributes preserved on fields
- **Debug impl**: on lock and guard types
- **Runtime-agnostic**: built on `async-lock`, works with tokio, async-std, smol, or any runtime
- **`#[must_use]`** on builder and guard types
- **`Send + Sync`** compile-time assertion on generated lock types
- **`Readable` / `Writable`** marker traits with `#[diagnostic::on_unimplemented]` for clearer error messages

[Unreleased]: https://github.com/mgild/smart-lock/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mgild/smart-lock/releases/tag/v0.1.0

# Contributing to smart-lock

Thank you for your interest in contributing!

## Getting Started

```bash
git clone https://github.com/mgild/smart-lock
cd smart-lock
cargo test
```

## Development

```bash
# Run all tests
cargo test

# Run clippy
cargo clippy --all-targets -- -D warnings

# Check formatting
cargo fmt --all -- --check

# Run miri tests (requires nightly)
cargo +nightly miri test --test miri

# Run benchmarks
cargo bench
```

## Pull Requests

- Keep PRs focused on a single change
- Add tests for new features
- Ensure `cargo test`, `cargo clippy`, and `cargo fmt` pass
- Update documentation if the public API changes
- Add a CHANGELOG entry under `## Unreleased`

## Reporting Issues

Please include:
- Rust version (`rustc --version`)
- Minimal reproduction case
- Expected vs actual behavior

## Stability Guarantees

- **MSRV policy**: The minimum supported Rust version (currently 1.78) is only bumped in minor or major releases, never in patch releases. MSRV bumps are noted in the CHANGELOG.
- **Generated type names**: The names `FooLock`, `FooLockBuilder`, and `FooLockGuard` (where `Foo` is the annotated struct) are part of the semver-stable public API.
- **Public API**: All items re-exported from `smart_lock::*` are covered by semver. Internal items behind `#[doc(hidden)]` are not.

## License

By contributing, you agree that your contributions will be dual-licensed under MIT OR Apache-2.0.

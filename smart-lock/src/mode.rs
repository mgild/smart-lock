/// Marker: field was not requested in the builder. No access available.
pub struct Unlocked;

/// Marker: field was requested for reading. Deref available, DerefMut not.
pub struct ReadLocked;

/// Marker: field was requested for writing. Both Deref and DerefMut available.
pub struct WriteLocked;

/// Marker: field was requested for upgradable reading. Deref available,
/// and the guard can be atomically upgraded to WriteLocked.
pub struct UpgradeLocked;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockModeKind {
    None,
    Read,
    Write,
    Upgrade,
}

pub trait LockMode {
    const MODE: LockModeKind;
}

impl LockMode for Unlocked {
    const MODE: LockModeKind = LockModeKind::None;
}

impl LockMode for ReadLocked {
    const MODE: LockModeKind = LockModeKind::Read;
}

impl LockMode for WriteLocked {
    const MODE: LockModeKind = LockModeKind::Write;
}

impl LockMode for UpgradeLocked {
    const MODE: LockModeKind = LockModeKind::Upgrade;
}

/// Implemented for lock modes that support reading (`ReadLocked`, `WriteLocked`, `UpgradeLocked`).
///
/// Not implemented for `Unlocked` — accessing an unlocked field is a compile error.
#[diagnostic::on_unimplemented(
    message = "cannot read from a field with `{Self}` access",
    note = "add `.read_field()` or `.write_field()` to the builder to lock this field"
)]
pub trait Readable {}

impl Readable for ReadLocked {}
impl Readable for WriteLocked {}
impl Readable for UpgradeLocked {}

/// Implemented only for `WriteLocked`.
///
/// Not implemented for `ReadLocked` or `UpgradeLocked` — mutating a read-locked field is a compile error.
#[diagnostic::on_unimplemented(
    message = "cannot write to a field with `{Self}` access",
    note = "use `.write_field()` instead of `.read_field()` to get mutable access"
)]
pub trait Writable {}

impl Writable for WriteLocked {}

/// Maps a lock mode to its "rest read" output for `lock_rest_read()`.
///
/// - `Unlocked` → `ReadLocked` (fill the gap with a read lock)
/// - `ReadLocked` → `ReadLocked` (identity)
/// - `WriteLocked` → `WriteLocked` (preserve explicit write)
/// - `UpgradeLocked` → `UpgradeLocked` (preserve explicit upgrade)
pub trait DefaultRead {
    type Output: LockMode;
}

impl DefaultRead for Unlocked {
    type Output = ReadLocked;
}

impl DefaultRead for ReadLocked {
    type Output = ReadLocked;
}

impl DefaultRead for WriteLocked {
    type Output = WriteLocked;
}

impl DefaultRead for UpgradeLocked {
    type Output = UpgradeLocked;
}

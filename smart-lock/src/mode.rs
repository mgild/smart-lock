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

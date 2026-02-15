use async_lock::{RwLock, RwLockReadGuard, RwLockUpgradableReadGuard, RwLockWriteGuard};
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use crate::mode::{
    LockMode, LockModeKind, ReadLocked, Readable, UpgradeLocked, Writable, WriteLocked,
};

enum FieldGuardInner<'a, T> {
    Read(RwLockReadGuard<'a, T>),
    Write(RwLockWriteGuard<'a, T>),
    Upgrade(RwLockUpgradableReadGuard<'a, T>),
    None,
}

/// A field guard whose access level is encoded in the type parameter `M`.
///
/// - `FieldGuard<'a, T, WriteLocked>` — `Deref` + `DerefMut`
/// - `FieldGuard<'a, T, ReadLocked>` — `Deref` only
/// - `FieldGuard<'a, T, UpgradeLocked>` — `Deref` only, can `.upgrade().await` to `WriteLocked`
/// - `FieldGuard<'a, T, Unlocked>` — no access (compile error on dereference)
pub struct FieldGuard<'a, T, M> {
    inner: FieldGuardInner<'a, T>,
    _mode: PhantomData<M>,
}

impl<'a, T, M> FieldGuard<'a, T, M> {
    /// Acquire the appropriate lock based on the mode's const discriminant.
    ///
    /// Dispatches to [`RwLock::read`], [`RwLock::write`], or
    /// [`RwLock::upgradable_read`] depending on `M::MODE`. For [`Unlocked`](crate::Unlocked)
    /// fields, returns a no-op guard without touching the lock.
    #[inline(always)]
    pub async fn acquire(lock: &'a RwLock<T>) -> Self
    where
        M: LockMode,
    {
        let inner = match M::MODE {
            LockModeKind::Write => FieldGuardInner::Write(lock.write().await),
            LockModeKind::Read => FieldGuardInner::Read(lock.read().await),
            LockModeKind::Upgrade => FieldGuardInner::Upgrade(lock.upgradable_read().await),
            LockModeKind::None => FieldGuardInner::None,
        };
        Self {
            inner,
            _mode: PhantomData,
        }
    }

    /// Try to acquire the appropriate lock without blocking.
    ///
    /// Returns `None` if the lock cannot be immediately acquired.
    /// [`Unlocked`](crate::Unlocked) fields always succeed (no lock touched).
    #[inline(always)]
    pub fn try_acquire(lock: &'a RwLock<T>) -> Option<Self>
    where
        M: LockMode,
    {
        let inner = match M::MODE {
            LockModeKind::Write => FieldGuardInner::Write(lock.try_write()?),
            LockModeKind::Read => FieldGuardInner::Read(lock.try_read()?),
            LockModeKind::Upgrade => FieldGuardInner::Upgrade(lock.try_upgradable_read()?),
            LockModeKind::None => FieldGuardInner::None,
        };
        Some(Self {
            inner,
            _mode: PhantomData,
        })
    }

    /// Create a no-op guard for [`Unlocked`](crate::Unlocked) fields.
    ///
    /// Zero-cost: no lock is acquired. Attempting to dereference an unlocked
    /// guard is a compile error (neither [`Deref`] nor [`DerefMut`] is implemented
    /// for `Unlocked`).
    #[inline(always)]
    pub fn unlocked() -> Self {
        Self {
            inner: FieldGuardInner::None,
            _mode: PhantomData,
        }
    }
}

// --- Upgrade: UpgradeLocked → WriteLocked (async, waits for readers to drain) ---
impl<'a, T> FieldGuard<'a, T, UpgradeLocked> {
    /// Atomically upgrade from upgradable read to exclusive write.
    ///
    /// Waits for all other readers to drain before granting write access.
    #[inline(always)]
    pub async fn upgrade(self) -> FieldGuard<'a, T, WriteLocked> {
        match self.inner {
            FieldGuardInner::Upgrade(g) => FieldGuard {
                inner: FieldGuardInner::Write(RwLockUpgradableReadGuard::upgrade(g).await),
                _mode: PhantomData,
            },
            _ => unreachable!(),
        }
    }
}

// --- Try upgrade: UpgradeLocked → WriteLocked (sync, non-blocking) ---
impl<'a, T> FieldGuard<'a, T, UpgradeLocked> {
    /// Try to upgrade from upgradable read to exclusive write without blocking.
    /// Returns `Ok(WriteLocked)` on success, `Err(self)` if readers are active.
    #[inline(always)]
    pub fn try_upgrade(self) -> Result<FieldGuard<'a, T, WriteLocked>, Self> {
        match self.inner {
            FieldGuardInner::Upgrade(g) => match RwLockUpgradableReadGuard::try_upgrade(g) {
                Ok(write_guard) => Ok(FieldGuard {
                    inner: FieldGuardInner::Write(write_guard),
                    _mode: PhantomData,
                }),
                Err(upgrade_guard) => Err(FieldGuard {
                    inner: FieldGuardInner::Upgrade(upgrade_guard),
                    _mode: PhantomData,
                }),
            },
            _ => unreachable!(),
        }
    }
}

// --- Downgrade: WriteLocked → ReadLocked (sync, atomic) ---
impl<'a, T> FieldGuard<'a, T, WriteLocked> {
    /// Atomically downgrade from exclusive write to shared read.
    ///
    /// Immediately allows other readers. Synchronous — no `.await` needed.
    #[inline(always)]
    pub fn downgrade(self) -> FieldGuard<'a, T, ReadLocked> {
        match self.inner {
            FieldGuardInner::Write(g) => FieldGuard {
                inner: FieldGuardInner::Read(RwLockWriteGuard::downgrade(g)),
                _mode: PhantomData,
            },
            _ => unreachable!(),
        }
    }
}

// --- Downgrade: UpgradeLocked → ReadLocked (sync, atomic) ---
impl<'a, T> FieldGuard<'a, T, UpgradeLocked> {
    /// Atomically downgrade from upgradable read to shared read.
    ///
    /// Releases the upgrade slot, allowing other tasks to acquire upgradable locks.
    /// Synchronous — no `.await` needed.
    #[inline(always)]
    pub fn downgrade(self) -> FieldGuard<'a, T, ReadLocked> {
        match self.inner {
            FieldGuardInner::Upgrade(g) => FieldGuard {
                inner: FieldGuardInner::Read(RwLockUpgradableReadGuard::downgrade(g)),
                _mode: PhantomData,
            },
            _ => unreachable!(),
        }
    }
}

// --- Debug ---

impl<T: fmt::Debug, M> fmt::Debug for FieldGuard<'_, T, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            FieldGuardInner::Read(g) => fmt::Debug::fmt(&**g, f),
            FieldGuardInner::Write(g) => fmt::Debug::fmt(&**g, f),
            FieldGuardInner::Upgrade(g) => fmt::Debug::fmt(&**g, f),
            FieldGuardInner::None => f.write_str("<unlocked>"),
        }
    }
}

// --- Deref: any Readable mode (ReadLocked, WriteLocked, UpgradeLocked) ---

impl<T, M: Readable> Deref for FieldGuard<'_, T, M> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        match &self.inner {
            FieldGuardInner::Read(g) => g,
            FieldGuardInner::Write(g) => g,
            FieldGuardInner::Upgrade(g) => g,
            FieldGuardInner::None => unreachable!(),
        }
    }
}

// --- DerefMut: WriteLocked only ---

impl<T, M: Writable + Readable> DerefMut for FieldGuard<'_, T, M> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        match &mut self.inner {
            FieldGuardInner::Write(g) => &mut *g,
            _ => unreachable!(),
        }
    }
}

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use async_lock::{RwLock, RwLockReadGuard, RwLockUpgradableReadGuard, RwLockWriteGuard};

use crate::mode::{LockMode, LockModeKind, ReadLocked, UpgradeLocked, WriteLocked};

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

    /// Create a no-op guard for unlocked fields. Zero-cost: no lock acquired.
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

// --- Downgrade: WriteLocked → ReadLocked (sync, atomic) ---
impl<'a, T> FieldGuard<'a, T, WriteLocked> {
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

// --- Deref impls ---

impl<T> Deref for FieldGuard<'_, T, ReadLocked> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        match &self.inner {
            FieldGuardInner::Read(g) => g,
            _ => unreachable!(),
        }
    }
}

impl<T> Deref for FieldGuard<'_, T, WriteLocked> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        match &self.inner {
            FieldGuardInner::Write(g) => g,
            _ => unreachable!(),
        }
    }
}

impl<T> Deref for FieldGuard<'_, T, UpgradeLocked> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        match &self.inner {
            FieldGuardInner::Upgrade(g) => g,
            _ => unreachable!(),
        }
    }
}

// --- DerefMut: WriteLocked only ---

impl<T> DerefMut for FieldGuard<'_, T, WriteLocked> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        match &mut self.inner {
            FieldGuardInner::Write(g) => &mut *g,
            _ => unreachable!(),
        }
    }
}

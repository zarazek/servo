/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Different objects protected by the same lock

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// Convenience type alias.
pub type ArcSharedRwLock<T> = Arc<SharedRwLock<T>>;

/// Object protected by a shared lock
pub struct SharedRwLock<T> {
    rwlock: Arc<RwLock<()>>,
    data: UnsafeCell<T>,
}

/// Proof that the lock is held for reading
pub struct ReadGuard<'a, T: 'a> {
    locked_data: &'a SharedRwLock<T>,
    inner_guard: ReadGuardInner<'a>,
}

/// Proof that the lock is held for writing
pub struct WriteGuard<'a, T: 'a> {
    locked_data: &'a SharedRwLock<T>,
    inner_guard: WriteGuardInner<'a>,
}

enum ReadGuardInner<'a> {
    Owned(RwLockReadGuard<'a, ()>),
    Ref(&'a RwLockReadGuard<'a, ()>),
    Downgraded(&'a RwLockWriteGuard<'a, ()>)
}

enum WriteGuardInner<'a> {
    Owned(RwLockWriteGuard<'a, ()>),
    RefMut(&'a mut RwLockWriteGuard<'a, ()>),
}

impl<T> SharedRwLock<T> {
    /// Create with a new shared RwLock
    pub fn new(data: T) -> Self {
        SharedRwLock {
            rwlock: Arc::new(RwLock::new(())),
            data: UnsafeCell::new(data),
        }
    }

    /// Create a new SharedRwLock with the same RwLock as `self`.
    ///
    /// A guard obtained by calling `.read()` or `.write()` on `self`
    /// can be used with the return value, or vice-versa.
    pub fn new_with_same_lock<U>(&self, data: U) -> SharedRwLock<U> {
        SharedRwLock {
            rwlock: self.rwlock.clone(),
            data: UnsafeCell::new(data),
        }
    }

    /// Acquire the shared lock and access this data for reading.
    pub fn read(&self) -> ReadGuard<T> {
        ReadGuard {
            locked_data: self,
            inner_guard: ReadGuardInner::Owned(self.rwlock.read()),
        }
    }

    /// Acquire the shared lock and access this data for writing.
    pub fn write(&self) -> WriteGuard<T> {
        WriteGuard {
            locked_data: self,
            inner_guard: WriteGuardInner::Owned(self.rwlock.write()),
        }
    }

    /// Access this data for reading with the shared lock already acquired.
    ///
    /// Return a new read guard with the lifetime of *both* the data and the existing guard:
    /// it can outlive neither.
    pub fn read_with<'a, U>(&'a self, existing_guard: &'a ReadGuard<U>) -> ReadGuard<'a, T> {
        assert!(same_rwlock(&*self.rwlock, &*existing_guard.locked_data.rwlock),
                "Calling SharedRwLock::read_with with a guard from an unrelated RwLock");
        ReadGuard {
            locked_data: self,
            inner_guard: match existing_guard.inner_guard {
                ReadGuardInner::Owned(ref g) => ReadGuardInner::Ref(g),
                ReadGuardInner::Ref(g) => ReadGuardInner::Ref(g),
                ReadGuardInner::Downgraded(ref g) => ReadGuardInner::Downgraded(&*g),
            },
        }
    }

    /// Access this data for write with the shared lock already acquired.
    ///
    /// Return a new write guard with the lifetime of *both* the data and the existing guard:
    /// it can outlive neither.
    pub fn write_with<'a, U>(&'a self, existing_guard: &'a mut WriteGuard<'a, U>)
                             -> WriteGuard<'a, T> {
        assert!(same_rwlock(&*self.rwlock, &*existing_guard.locked_data.rwlock),
                "Calling SharedRwLock::write_with with a guard from an unrelated RwLock");
        WriteGuard {
            locked_data: self,
            inner_guard: match existing_guard.inner_guard {
                WriteGuardInner::Owned(ref mut g) => WriteGuardInner::RefMut(g),
                WriteGuardInner::RefMut(ref mut g) => WriteGuardInner::RefMut(&mut **g),
            },
        }
    }
}

fn same_rwlock(a: *const RwLock<()>, b: *const RwLock<()>) -> bool {
    a == b
}

impl<'a, T> WriteGuard<'a, T> {
    /// Return a read guard that references a write guard
    pub fn downgrade(&self) -> ReadGuard<T> {
        ReadGuard {
            locked_data: self.locked_data,
            inner_guard: ReadGuardInner::Downgraded(match self.inner_guard {
                WriteGuardInner::Owned(ref g) => g,
                WriteGuardInner::RefMut(ref g) => &**g,
            }),
        }
    }
}

impl<'a, T> Deref for ReadGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // Exercise the borrow checker to ensure we do have a valid guard.
        let _: &() = match self.inner_guard {
            ReadGuardInner::Owned(ref g) => &**g,
            ReadGuardInner::Ref(g) => &**g,
            ReadGuardInner::Downgraded(g) => &**g,
        };
        unsafe {
            &*self.locked_data.data.get()
        }
    }
}

impl<'a, T> Deref for WriteGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // Exercise the borrow checker to ensure we do have a valid guard.
        let _: &() = match self.inner_guard {
            WriteGuardInner::Owned(ref g) => &**g,
            WriteGuardInner::RefMut(ref g) => &***g,
        };
        unsafe {
            &*self.locked_data.data.get()
        }
    }
}

impl<'a, T> DerefMut for WriteGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // Exercise the borrow checker to ensure we do have a valid write guard.
        let _: &mut () = match self.inner_guard {
            WriteGuardInner::Owned(ref mut g) => &mut **g,
            WriteGuardInner::RefMut(ref mut g) => &mut ***g,
        };
        unsafe {
            &mut *self.locked_data.data.get()
        }
    }
}

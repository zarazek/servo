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
    inner_guard: MaybeRef<'a, RwLockReadGuard<'a, ()>>,
}

/// Proof that the lock is held for writing
pub struct WriteGuard<'a, T: 'a> {
    locked_data: &'a SharedRwLock<T>,
    inner_guard: MaybeRefMut<'a, RwLockWriteGuard<'a, ()>>,
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
            inner_guard: MaybeRef::Owned(self.rwlock.read()),
        }
    }

    /// Acquire the shared lock and access this data for writing.
    pub fn write(&self) -> WriteGuard<T> {
        WriteGuard {
            locked_data: self,
            inner_guard: MaybeRefMut::Owned(self.rwlock.write()),
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
            inner_guard: MaybeRef::Ref(&existing_guard.inner_guard),
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
            inner_guard: MaybeRefMut::RefMut(&mut existing_guard.inner_guard),
        }
    }
}

fn same_rwlock(a: *const RwLock<()>, b: *const RwLock<()>) -> bool {
    a == b
}

impl<'a, T> Deref for ReadGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // Exercise the borrow checker to ensure we do have a valid guard.
        let _: &() = &**self.inner_guard;
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
        let _: &() = &**self.inner_guard;
        unsafe {
            &*self.locked_data.data.get()
        }
    }
}

impl<'a, T> DerefMut for WriteGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // Exercise the borrow checker to ensure we do have a valid write guard.
        let _: &mut () = &mut **self.inner_guard;
        unsafe {
            &mut *self.locked_data.data.get()
        }
    }
}

enum MaybeRef<'a, T: 'a> {
    Owned(T),
    Ref(&'a T),
}

enum MaybeRefMut<'a, T: 'a> {
    Owned(T),
    RefMut(&'a mut T),
}


impl<'a, T> Deref for MaybeRef<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        match *self {
            MaybeRef::Owned(ref x) => x,
            MaybeRef::Ref(ref x) => x,
        }
    }
}

impl<'a, T> Deref for MaybeRefMut<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        match *self {
            MaybeRefMut::Owned(ref x) => x,
            MaybeRefMut::RefMut(ref x) => x,
        }
    }
}

impl<'a, T> DerefMut for MaybeRefMut<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        match *self {
            MaybeRefMut::Owned(ref mut x) => x,
            MaybeRefMut::RefMut(ref mut x) => x,
        }
    }
}

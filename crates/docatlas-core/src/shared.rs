//! Contains shared object defintitions

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// A shared, thread safe object
#[derive(Debug, Default)]
pub struct Shared<T> {
    inner: Arc<RwLock<T>>,
}

impl<T> Shared<T> {
    /// Creates a new shared value
    pub fn new(v: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(v)),
        }
    }

    pub fn read(&self) -> SharedReadGuard<T> {
        SharedReadGuard {
            guard: self.inner.read(),
        }
    }

    pub fn try_read(&self) -> Option<SharedReadGuard<T>> {
        self.inner.try_read().map(|guard| SharedReadGuard { guard })
    }

    pub fn write(&self) -> SharedWriteGuard<T> {
        SharedWriteGuard {
            guard: self.inner.write(),
        }
    }

    pub fn try_write(&self) -> Option<SharedWriteGuard<T>> {
        self.inner
            .try_write()
            .map(|guard| SharedWriteGuard { guard })
    }
}

/// Determines whether two shared pointers reference the same object
impl<T> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        let x: *const RwLock<T> = &*self.inner;
        let y: *const RwLock<T> = &*other.inner;
        x == y
    }
}

impl<T> Eq for Shared<T> {}

impl<T> Hash for Shared<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let x: *const RwLock<T> = &*self.inner;
        x.hash(state)
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// A read guard of a shared value.
#[derive(Debug)]
pub struct SharedReadGuard<'a, T> {
    guard: RwLockReadGuard<'a, T>,
}

impl<T> Deref for SharedReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

/// A read guard of a shared value.
#[derive(Debug)]
pub struct SharedWriteGuard<'a, T> {
    guard: RwLockWriteGuard<'a, T>,
}

impl<T> Deref for SharedWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<T> DerefMut for SharedWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}

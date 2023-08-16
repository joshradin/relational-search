//! Describes the lock api used for managing authorization

use std::fmt::{Debug, Formatter};
use rand::random;

/// A key that's used to open a [lock](Lock).
///
/// Mostly used as a marker trait.
pub trait Key {}

/// The "mechanism" that's used to verify a [key](Key) within a [lock](Lock)
pub trait Tumbler {
    type Key: Key;
    type Error;

    /// Gets whether this key unlocks the given tumlber
    fn unlock(&self, key: &Self::Key) -> Result<(), Self::Error>;
}

/// Wrapper around some data that can be only be accessed with the correct [key](Key) for its
/// [tumbler](Tumbler) mechanism.
pub struct Lock<T, M: Tumbler> {
    tumbler: M,
    data: T,
}

impl<T, M: Tumbler> Debug for Lock<T, M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lock")
            .finish_non_exhaustive()
    }
}

impl<T, M: Tumbler> Lock<T, M> {
    /// Creates a new lock that contains some data
    pub fn new(tumbler: M, data: T) -> Self {
        Self { tumbler, data }
    }

    /// Tries to get a reference to the inner data with a given key
    pub fn get(&self, key: &M::Key) -> Result<&T, M::Error> {
        self.tumbler.unlock(key).map(|()| &self.data)
    }

    /// Tries to get a mutable reference to the inner data with a given key
    pub fn get_mut(&mut self, key: &M::Key) -> Result<&mut T, M::Error> {
        self.tumbler.unlock(key).map(|()| &mut self.data)
    }

    /// Attempts to take the inner data if the key is matching
    pub fn take(self, key: &M::Key) -> Result<T, (M::Error, Self)> {
        match self.tumbler.unlock(key) {
            Ok(()) => {
                Ok(self.data)
            }
            Err(e) => {
                Err((e, self))
            }
        }
    }
}

/// Simple key
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct SimpleKey<const N: usize>([u8; N]);

impl<const N: usize> Key for SimpleKey<N> {}

impl<const N: usize> Tumbler for SimpleKey<N> {
    type Key = Self;
    type Error = ();

    fn unlock(&self, key: &Self::Key) -> Result<(), Self::Error> {
        match self == key {
            true => Ok(()),
            false => Err(()),
        }
    }
}

impl<const N: usize> SimpleKey<N> {
    pub fn new() -> Self {
        let mut arr = [0; N];
        for i in 0..N {
            arr[i] = random();
        }
        Self(arr)
    }
}

#[cfg(test)]
mod tests {
    use crate::auth::lock_api::{Lock, SimpleKey};

    #[test]
    fn simple_lock() {
        let key = SimpleKey::<16>::new();
        let lock = Lock::new(key.clone(), "Hello, World!");
        println!("lock: {lock:#?}");
        assert_eq!(lock.get(&key).unwrap(), &"Hello, World!");
        assert!(lock.get(&SimpleKey::<16>::new()).is_err());
    }
}

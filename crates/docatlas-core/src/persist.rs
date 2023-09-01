//! Memory stuff

mod block;
mod persisted_cell;
mod persisted_unsafe_cell;
mod persisted_vec;
mod persisted_raw_array;

pub use {
    persisted_cell::PersistedCell, persisted_unsafe_cell::PersistedUnsafeCell,
    persisted_vec::PersistentVec,
};

/// A marker trait for types that can be persisted
pub trait Persist {}

impl<T: Copy> Persist for T {}

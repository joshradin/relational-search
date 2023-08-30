//! Memory stuff


mod block;
mod persisted_unsafe_cell;
mod persisted_cell;
mod persisted_vec;

pub use {
    persisted_unsafe_cell::PersistedUnsafeCell,
    persisted_cell::PersistedCell,
};
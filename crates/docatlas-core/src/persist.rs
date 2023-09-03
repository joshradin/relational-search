//! Memory stuff

mod block;
mod persisted_cell;
mod persisted_raw_array;
mod persisted_unsafe_cell;
mod persisted_vec;

pub use {
    persisted_cell::PersistedCell, persisted_unsafe_cell::PersistedUnsafeCell,
    persisted_vec::PersistentVec,
};

/// A marker trait for types that can be persisted
pub trait Persist: Copy {
    unsafe fn write_ptr(src: *const Self, dest: *mut u8);
    unsafe fn read(src: *const u8) -> *const Self;
}

impl<T: Copy> Persist for T {
    unsafe fn write_ptr(src: *const Self, dest: *mut u8) {
        std::ptr::copy(src, dest as *mut T, 1)
    }

    unsafe fn read(src: *const u8) -> *const Self {
        src as *const T
    }
}

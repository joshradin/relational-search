use crate::persist::block::Block;
use crate::persist::{Persist, PersistedUnsafeCell};

/// A mutable, persisted cell that allows for safe usage.
///
/// Implements PartialEq, PartialOrd, Eq, Ord, and Hash
#[derive(Debug)]
pub struct PersistedCell<T: Persist> {
    unsafe_cell: PersistedUnsafeCell<T>,
}

impl<T: Persist> PersistedCell<T> {
    /// A persisted cell
    pub fn new(block: Block, value: T) -> Self {
        Self {
            unsafe_cell: PersistedUnsafeCell::new(block, value),
        }
    }

    /// Gets a mutable reference to the underlying data
    pub fn get_mut(&mut self) -> &mut T {
        self.unsafe_cell.get_mut()
    }

    /// Sets the wrapped value
    pub fn set(&self, val: T) {
        unsafe {
            *self.unsafe_cell.get() = val;
        }
    }

    /// Swaps the values in two cells
    pub fn swap(&self, other: &Self) {
        unsafe { std::ptr::swap(self.unsafe_cell.get(), other.unsafe_cell.get()) }
    }

    /// Replaces the value wrapped in this cell with a new value.
    ///
    /// Returns the old value of the cell.
    pub fn replace(&self, value: T) -> T {
        unsafe { std::ptr::replace(self.unsafe_cell.get(), value) }
    }

    /// Unwraps the inner value of this cell
    pub fn into_inner(self) -> T {
        self.unsafe_cell.into_inner()
    }
}

impl<T: Copy + Persist> PersistedCell<T> {
    /// Copies the wrapped value of this persisted cell
    pub fn get(&self) -> T {
        unsafe { *self.unsafe_cell.get() }
    }
}

impl<T: Default + Persist> PersistedCell<T> {
    /// Takes the wrapped value of this persisted cell, leaving [`Default::default`](Default::default)
    /// in it's place.
    pub fn take(&self) -> T {
        self.replace(T::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persist::block::Blocks;

    #[test]
    fn test_new() {
        let block = Blocks.new();
        let cell = PersistedCell::new(block, 32);
    }

    #[test]
    fn test_swap() {
        let cell1 = PersistedCell::new(Blocks.new(), 32);
        let cell2 = PersistedCell::new(Blocks.new(), 64);

        cell1.swap(&cell2);
        assert_eq!(cell1.get(), 64);
        assert_eq!(cell2.get(), 32);
    }

    #[test]
    fn test_replace() {
        let cell = PersistedCell::new(Blocks.new(), 32);
        let replaced = cell.replace(64);
        assert_eq!(replaced, 32);
        assert_eq!(cell.get(), 64);
    }

    #[test]
    fn test_take() {
        let cell = PersistedCell::new(Blocks.new(), 32);
        let took = cell.take();
        assert_eq!(took, 32);
        assert_eq!(cell.get(), 0);
    }
}

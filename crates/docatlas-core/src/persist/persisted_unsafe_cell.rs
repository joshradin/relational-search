use std::marker::PhantomData;
use crate::persist::block::Block;
use crate::persist::Persist;

#[derive(Debug)]
pub struct PersistedUnsafeCell<T: Persist> {
    block: Block,
    _kind: PhantomData<T>
}

impl<T: Persist> PersistedUnsafeCell<T> {
    /// Creates a new unsafe cell with a given value
    pub fn new(mut block: Block, value: T) -> Self {
        unsafe {
            block.assert_can_contain::<T>();
            std::ptr::write(block.as_ptr_mut() as *mut T, value);
            Self { block, _kind: PhantomData }
        }
    }

    /// Unwraps the value, consuming the cell.
    pub fn into_inner(self) -> T {
        unsafe { std::ptr::read(self.block.as_aligned_ptr()) }
    }

    /// Gets a mutable to pointer to the wrapped value
    pub fn get(&self) -> *mut T {
        unsafe { self.block.as_aligned_ptr::<T>() as *mut T }
    }

    /// Gets a mutable reference to the persisted unsafe cell
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.block.as_aligned_ptr_mut::<T>() }
    }
}

impl<T: Persist> Drop for PersistedUnsafeCell<T> {
    fn drop(&mut self) {
        unsafe {
            let read = std::ptr::read(self.block.as_ptr());
            drop(read)
        }
    }
}

use crate::persist::block::{Block, Persist};

#[derive(Debug)]
pub struct PersistedUnsafeCell<T: Persist> {
    block: Block<T>
}

impl<T: Persist> PersistedUnsafeCell<T> {

    /// Creates a new unsafe cell with a given value
    pub fn new(mut block: Block<T>, value: T) -> Self {
        unsafe {
            std::ptr::write(block.as_mut_ptr(), value);
            Self {
                block
            }
        }
    }

    /// Unwraps the value, consuming the cell.
    pub fn into_inner(self) -> T {
        unsafe { std::ptr::read(self.block.as_ptr()) }
    }

    /// Gets a mutable to pointer to the wrapped value
    pub fn get(&self) -> *mut T {
        self.block.as_ptr() as *mut T
    }

    /// Gets a mutable reference to the persisted unsafe cell
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.block.as_mut_ptr() }
    }
}
use std::marker::PhantomData;
use std::ops::Add;

use crate::persist::block::Block;
use crate::persist::Persist;

///! The persisted raw array
#[derive(Debug)]
pub struct PersistedRawArray<T: Persist> {
    block: Block,
    offset: usize,
    _kind: PhantomData<T>,
}

impl<T: Persist> PersistedRawArray<T> {
    /// Creates a new raw array from a given block, with the array starting at an optional offset from
    /// the beginning of the block.
    pub unsafe fn new(block: Block) -> PersistedRawArray<T> {
        let ptr = block.as_ptr();
        let alignment = std::mem::align_of::<T>();
        let offset = ptr.align_offset(alignment);

        Self {
            block,
            offset,
            _kind: PhantomData,
        }
    }

    /// Gets the capacity of the raw array.
    pub fn capacity(&self) -> usize {
        self.block.size() - self.offset
    }

    /// Reserves an additional amount of space required to store at least `additional` more
    /// `T` values.
    pub unsafe fn reserve(&mut self, additional: usize) {
        let target_capacity = self.capacity() + additional;
        while self.capacity() < target_capacity {
            let additional_bytes = std::mem::size_of::<T>() * additional;
            self.block.reserve(additional_bytes);
            let alignment = std::mem::align_of::<T>();
            let offset = self.block.as_ptr().align_offset(alignment);
            self.offset = offset;
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn create_raw_array() {}
}

//! Memory stuff

use std::ops::{Deref, DerefMut};

pub use {
    persisted_cell::PersistedCell,
    persisted_unsafe_cell::PersistedUnsafeCell,
    persisted_vec::{Drain, PersistentVec, Split, SplitMut},
};

mod block;
mod persisted_box;
mod persisted_cell;
mod persisted_raw_array;
mod persisted_unsafe_cell;
mod persisted_vec;

/// A marker trait for types that can be persisted
pub trait Persist {
    /// Gets the minimum size of the type
    fn size() -> usize;

    /// Gets the size of this value
    fn size_of(&self) -> usize;
}

impl<T: Copy> Persist for T {
    fn size() -> usize {
        std::mem::size_of::<T>()
    }

    fn size_of(&self) -> usize {
        std::mem::size_of_val(self)
    }
}

#[derive(Debug)]
pub struct PData<T: Copy + Sized>([T]);
impl<T: Copy + Sized> PData<T> {
    pub fn write<I: IntoIterator<Item = T>>(dest: &mut [u8], data: I) -> &mut Self {
        let data = data.into_iter().collect::<Vec<_>>();
        if dest.len() < std::mem::size_of::<usize>() + std::mem::size_of::<T>() * data.len() {
            panic!("not enough space to write PData")
        }

        unsafe {
            let ptr = dest.as_mut_ptr();
            let len_ptr = ptr as *mut usize;
            *len_ptr = data.len();
            let data_ptr = len_ptr.add(1) as *mut T;
            std::ptr::copy(data.as_ptr(), data_ptr, data.len());

            let slice_ptr = std::ptr::slice_from_raw_parts(data_ptr, data.len());
            &mut *(slice_ptr as *const PData<T> as *mut _)
        }
    }

    pub fn write_singleton(dest: &mut [u8], data: T) -> &mut Self {
        Self::write(dest, [data])
    }

    pub fn read(src: &[u8]) -> &Self {
        unsafe {
            let ptr = src.as_ptr();
            let len_ptr = ptr as *const usize;
            let len = *len_ptr;
            let data_ptr = len_ptr.add(1) as *const T;

            let slice_ptr = std::ptr::slice_from_raw_parts(data_ptr, len);
            &*(slice_ptr as *const PData<T>)
        }
    }

    pub fn read_mut(src: &mut [u8]) -> &mut Self {
        unsafe {
            let ptr = src.as_ptr();
            let len_ptr = ptr as *const usize;
            let len = *len_ptr;
            let data_ptr = len_ptr.add(1) as *const T as *mut T;

            let slice_ptr = std::ptr::slice_from_raw_parts_mut(data_ptr, len);
            &mut *(slice_ptr as *mut PData<T>)
        }
    }
}

impl<T: Copy + Sized> AsRef<[T]> for PData<T> {
    fn as_ref(&self) -> &[T] {
        &self.0
    }
}

impl<T: Copy + Sized> AsMut<[T]> for PData<T> {
    fn as_mut(&mut self) -> &mut [T] {
        &mut self.0
    }
}

impl<T: Copy + Sized> Deref for PData<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Copy + Sized> DerefMut for PData<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Copy + Sized> Persist for PData<T> {
    fn size() -> usize {
        usize::size()
    }

    fn size_of(&self) -> usize {
        Self::size() + self.0.len() * T::size()
    }
}

#[cfg(test)]
mod tests {
    use std::iter;

    use crate::persist::block::Blocks;
    use crate::persist::{PData, PersistentVec};

    #[test]
    fn create_pdata_in_buffer() {
        let mut buffer = [0u8; 16];
        let pdata: &PData<i32> = PData::write(&mut buffer, [4, -5]);

        assert_eq!(pdata.len(), 2);
        assert_eq!(pdata.as_ref(), &[4, -5]);
    }

    #[test]
    fn persistent_vec_pdata() {
        let block = Blocks.new();
        let mut p_vec: PersistentVec<u8> = PersistentVec::new(block);
        p_vec.extend(iter::repeat(0).take(32));

        let split = p_vec.split_mut(16);
        assert_eq!(split.len(), 2);

        let lower = PData::<i32>::write_singleton(split.write(0).unwrap(), 32);
        let upper = PData::<i32>::write_singleton(split.write(1).unwrap(), 128);

        println!("{:?}", p_vec);
    }
}

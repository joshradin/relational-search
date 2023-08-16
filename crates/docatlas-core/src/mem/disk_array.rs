//! disk-backed array

use memmap::MmapMut;
use std::fs::File;
use std::io;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use crate::prelude::*;

/// A disk array, contains N units of T
#[derive(Debug)]
pub struct RawDiskArray<T> {
    _kind: PhantomData<T>,
    capacity: usize,
    mmap: MmapMut,
}

impl<T> RawDiskArray<T> {
    /// Store an array at a path
    pub unsafe fn new(len: usize, file: &File) -> Result<Self, io::Error> {
        file.set_len((std::mem::size_of::<T>() * len) as u64)?;
        let mut mmap = MmapMut::map_mut(&file)?;
        Ok(Self {
            _kind: PhantomData,
            capacity: len,
            mmap,
        })
    }

    /// Gets the total capacity of the array
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// as const pointer
    pub fn as_ptr(&self) -> *const MaybeUninit<T> {
        self.mmap.as_ptr() as *const MaybeUninit<T>
    }

    /// as mut pointer
    pub fn as_mut_ptr(&mut self) -> *mut MaybeUninit<T> {
        self.mmap.as_mut_ptr() as *mut MaybeUninit<T>
    }

    /// Takes the values within the disk backed vector
    pub fn take(self) -> Vec<MaybeUninit<T>> {
        unsafe {
            self.iter()
                .map(|v| {
                    let mut new = MaybeUninit::uninit();
                    std::ptr::copy(v.as_ptr(), new.as_mut_ptr(), 1);
                    new
                })
                .collect()
        }
    }
}

impl<T> AsRef<[MaybeUninit<T>]> for RawDiskArray<T> {
    fn as_ref(&self) -> &[MaybeUninit<T>] {
        unsafe { &*std::slice::from_raw_parts(self.as_ptr(), self.capacity) }
    }
}

impl<T> AsMut<[MaybeUninit<T>]> for RawDiskArray<T> {
    fn as_mut(&mut self) -> &mut [MaybeUninit<T>] {
        unsafe { &mut *std::slice::from_raw_parts_mut(self.as_mut_ptr(), self.capacity) }
    }
}

impl<T> Deref for RawDiskArray<T> {
    type Target = [MaybeUninit<T>];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T> DerefMut for RawDiskArray<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl<T> Drop for RawDiskArray<T> {
    fn drop(&mut self) {
        drop(self.mmap.flush());
    }
}


#[cfg(test)]
mod tests {
    use crate::mem::disk_array::RawDiskArray;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn can_create_from_array() {
        let dir = tempdir().unwrap();
        let array: RawDiskArray<&str> = unsafe {
            let path = dir.path().join("temp@1");
            let mut array = RawDiskArray::new(
                2,
                &File::options()
                    .create(true)
                    .read(true)
                    .write(true)
                    .open(path)
                    .unwrap(),
            )
                .unwrap();
            *array[0].as_mut_ptr() = "hello";
            *array[1].as_mut_ptr() = "world";
            array
        };

        unsafe {
            assert_eq!(array.capacity, 2);
            assert_eq!(array[0].assume_init(), "hello");
            assert_eq!(array[1].assume_init(), "world");
            assert!(array.get(2).is_none());
        }
    }
}

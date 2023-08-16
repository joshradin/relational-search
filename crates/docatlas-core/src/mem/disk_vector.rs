use crate::mem::disk_array::RawDiskArray;
use std::fs::File;
use std::io;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};

use crate::prelude::*;

/// A disk-backed vector
#[derive(Debug)]
pub struct DiskVector<T> {
    len: usize,
    file: File,
    raw_array: Option<RawDiskArray<T>>,
}

/// The default disk vector capacity
pub const DEFAULT_DISK_VECTOR_CAPACITY: usize = 8;

impl<T> DiskVector<T> {
    pub unsafe fn new(file: File) -> Result<Self, io::Error> {
        Self::with_capacity(0, file)
    }

    /// Creates a new disk-backed vector
    pub unsafe fn with_capacity(cap: usize, file: File) -> Result<Self, io::Error> {
        let raw = if cap > 0 {
            Some(RawDiskArray::new(cap, &file)?)
        } else {
            None
        };
        Ok(Self {
            len: 0,
            file,
            raw_array: raw,
        })
    }

    /// Gets the capacity of the array
    pub fn capacity(&self) -> usize {
        self.raw_array
            .as_ref()
            .map(RawDiskArray::capacity)
            .unwrap_or(0)
    }

    /// Pushes a value onto the vector
    pub fn push(&mut self, value: T) {
        if self.len() == self.capacity() {
            // expand
            unsafe {
                let new_len = if self.len > 0 {
                    self.len * 2 - self.len
                } else {
                    DEFAULT_DISK_VECTOR_CAPACITY
                };
                self.reserve(new_len).expect("reservation failed");
            }
        }

        let uninit = &mut self.raw_array.as_mut().unwrap().as_mut()[self.len];
        unsafe {
            *uninit.as_mut_ptr() = value;
        }
        self.len += 1;
    }

    /// Removes the last value in the vector
    pub fn pop(&mut self) -> Option<T> {
        if self.len > 0 {
            let mut out: MaybeUninit<T> = MaybeUninit::uninit();
            unsafe {
                let ptr = &self.raw_array.as_ref().unwrap().as_ref()[self.len - 1];
                std::ptr::copy(ptr, &mut out, 1);
                self.len -= 1;
                return Some(out.assume_init());
            }
        } else {
            None
        }
    }

    /// Reserves an additional amount of space
    pub unsafe fn reserve(&mut self, additional: usize) -> Result<(), io::Error> {
        let new_raw = RawDiskArray::new(self.len + additional, &self.file)?;
        self.raw_array = Some(new_raw);
        Ok(())
    }

    /// Gets this disk vector as a slice
    pub fn as_slice(&self) -> &[T] {
        if self.len == 0 {
            return &[];
        }
        unsafe {
            let array_uninit = self.raw_array.as_ref().unwrap().as_ref();
            let slice = &(*array_uninit)[..self.len];
            std::mem::transmute(slice)
        }
    }

    /// Gets this disk vector as a mutable slice
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        if self.len == 0 {
            return &mut [];
        }
        unsafe {
            let array_uninit = self.raw_array.as_mut().unwrap().as_mut();
            let slice = &mut (*array_uninit)[..self.len];
            std::mem::transmute(slice)
        }
    }
}

impl<T> AsRef<[T]> for DiskVector<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> AsMut<[T]> for DiskVector<T> {
    fn as_mut(&mut self) -> &mut [T] {
        self.as_slice_mut()
    }
}

impl<T> Deref for DiskVector<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<T> DerefMut for DiskVector<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tempfile::tempfile;

    #[test]
    fn can_read_only_valid_indices() {
        let temp = tempfile().unwrap();
        unsafe {
            let mut vector = DiskVector::new(temp).unwrap();
            for i in 0..8 {
                vector.push(i);
                for j in 0..8 {
                    if j <= i {
                        assert_eq!(vector.get(j), Some(&j))
                    } else {
                        assert_eq!(vector.get(j), None)
                    }
                }
            }
        }
    }

    #[test]
    fn can_reserve_additional() {
        let temp = tempfile().unwrap();
        unsafe {
            let mut vector = DiskVector::with_capacity(1, temp).unwrap();
            for i in 0..8 {
                vector.push(i);
                for j in 0..8 {
                    if j <= i {
                        assert_eq!(vector.get(j), Some(&j))
                    } else {
                        assert_eq!(vector.get(j), None)
                    }
                }
            }
            assert!(vector.capacity() >= 8);
        }
    }

    #[test]
    fn can_pop() {
        let temp = tempfile().unwrap();
        unsafe {
            let mut vector = DiskVector::new(temp).unwrap();
            for i in 0..8 {
                vector.push(i);
            }

            for i in (0..8).rev() {
                assert_eq!(vector.pop(), Some(i));
            }
        }
    }
}

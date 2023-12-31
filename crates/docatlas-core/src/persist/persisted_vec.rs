//! A persisted vector

use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;
use std::mem::transmute;
use std::ops::{Bound, Deref, DerefMut, Not, RangeBounds};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::vec;

use crate::persist::block::{Block, Blocks};
use crate::persist::Persist;

/// A persistent vector.
///
/// Designed to simulate the std [`Vec`](std::vec::Vec) as closely as possible.
pub struct PersistentVec<T: Persist + ?Sized> {
    block: Block,
    _kind: PhantomData<T>,
}

#[derive(Debug)]
struct RawVec<T: Persist>([T]);

impl<T: Persist> Persist for RawVec<T> {
    fn size() -> usize {
        usize::size()
    }

    fn size_of(&self) -> usize {
        usize::size() + self.0.len() * T::size()
    }
}

impl<T: Persist> PersistentVec<T> {
    /// Creates a persistent vector in-memory
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::new(Blocks.new())
    }

    /// Creates a new persistent vector on a given block.
    pub fn new(block: Block) -> Self {
        unsafe {
            block.assert_can_contain::<usize>();
        }

        Self {
            block,
            _kind: PhantomData,
        }
    }

    unsafe fn as_raw_vec(&self) -> *const RawVec<T> {
        let len = *self.block.as_typed_ptr::<usize>();
        let data_ptr = self.block.as_typed_ptr::<usize>().add(1) as *const T;
        transmute(std::ptr::slice_from_raw_parts(data_ptr, len))
    }

    unsafe fn as_raw_vec_mut(&mut self) -> *mut RawVec<T> {
        let len = *self.block.as_typed_ptr::<usize>();
        let data_ptr = self.block.as_typed_mut_ptr::<usize>().add(1) as *mut T;
        transmute(std::ptr::slice_from_raw_parts_mut(data_ptr, len))
    }

    /// Creates a new persistent vector on a given block.
    pub fn with_iter<I: IntoIterator<Item = T>>(block: Block, iter: I) -> Self {
        let mut out = Self::new(block);
        out.extend(iter);
        out
    }

    /// Gets the capacity of the persistent vector
    pub fn capacity(&self) -> usize {
        unsafe {
            self.block.size()
                - ((self.as_data_ptr() as *const u8).offset_from(self.block.as_ptr())) as usize
        }
    }

    /// Reserves an additional amount of capacity to store `n` amount of `T`
    pub fn reserve(&mut self, capacity: usize) {
        unsafe {
            self.block.reserve(capacity * T::size());
        }
    }

    /// Gets the vector as a slice
    pub fn as_slice(&self) -> &[T]
    where
        T: Sized,
    {
        unsafe { &(*self.as_raw_vec()).0 }
    }

    /// Gets this vector as a mutable slice
    pub fn as_slice_mut(&mut self) -> &mut [T]
    where
        T: Sized,
    {
        unsafe { &mut (*self.as_raw_vec_mut()).0 }
    }

    /// Gets the length of the vector
    pub fn len(&self) -> usize {
        unsafe { *self.block.as_typed_ptr::<usize>() }
    }

    fn set_len(&mut self, len: usize) {
        unsafe {
            let len_ptr = self.block.as_typed_mut_ptr::<usize>();
            *len_ptr = len;
        }
    }

    fn as_data_ptr(&self) -> *const T {
        self.as_slice().as_ptr()
    }

    fn as_data_ptr_mut(&mut self) -> *mut T {
        self.as_slice_mut().as_mut_ptr()
    }

    /// Pushes a value to the end of the vector
    pub fn push(&mut self, value: T) {
        while self.len() + 1 >= self.capacity() {
            unsafe {
                // doubles the block size while the len is greater or equal to the capacity, and
                // makes sure the added value can be stored.
                let capacity = self.capacity();
                self.block.reserve(capacity * T::size() + value.size_of());
            }
        }

        unsafe {
            std::ptr::write(self.as_data_ptr_mut().add(self.len()), value);
        }
        self.set_len(self.len() + 1);
    }

    /// Pops the last value added to the vector
    pub fn pop(&mut self) -> Option<T> {
        if self.len() > 0 {
            unsafe {
                let value = std::ptr::read(self.as_data_ptr().add(self.len() - 1));
                self.set_len(self.len() - 1);
                Some(value)
            }
        } else {
            None
        }
    }

    /// Removes the value at the given index.
    ///
    /// # Panic
    /// Panics if index is greater than or equal to the length of the vector
    pub fn remove(&mut self, index: usize) -> T {
        if index >= self.len() {
            panic!("index must not >= {} (index: {index})", self.len())
        }

        if index == self.len() - 1 {
            return self.pop().unwrap();
        }

        unsafe {
            let start = self.as_data_ptr_mut().add(index);

            let out = std::ptr::read(start);

            let next = start.add(1);
            let move_count = self.len() - (index + 1);

            std::ptr::copy(next, start, move_count);
            self.set_len(self.len() - 1);

            out
        }
    }

    /// Clears all values stored in this persistent vector
    pub fn clear(&mut self) {
        while let Some(popped) = self.pop() {
            drop(popped);
        }
    }

    /// Retains only elements specified by the predicate.
    ///
    /// In other words, this removed all values where the predicate returns false.
    pub fn retain<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&T) -> bool,
    {
        for i in (0..self.len()).rev() {
            if let Some(value) = self.get(i) {
                if !predicate(value) {
                    self.remove(i);
                }
            }
        }
    }

    /// Drains the given range from the vector.
    ///
    /// # Panic
    /// Panics if the lower bound is greater than the upper bound, or if the end bound is greater than
    /// the length of the vector
    pub fn drain<R>(&mut self, range: R) -> Drain<T>
    where
        R: RangeBounds<usize> + 'static,
    {
        Drain::new(self, range)
    }

    /// Splits the data into slices of a given length
    pub fn split(&self, len: usize) -> Split<T> {
        Split {
            src: self,
            split_size: len,
        }
    }

    /// Gains access over the vector as a set of splits with a given length.
    ///
    /// Each split can be accessed individually to manipulate data within them.
    pub fn split_mut(&mut self, len: usize) -> SplitMut<T> {
        SplitMut::new(self, len)
    }
}

impl<T: Debug + Persist> Debug for PersistentVec<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistentVec")
            .field("block", &self.block)
            .field("slice", &self.as_slice())
            .finish()
    }
}

impl<T: Persist> AsRef<[T]> for PersistentVec<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T: Persist> AsMut<[T]> for PersistentVec<T> {
    fn as_mut(&mut self) -> &mut [T] {
        self.as_slice_mut()
    }
}

impl<T: Persist> Deref for PersistentVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T: Persist> DerefMut for PersistentVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_slice_mut()
    }
}

impl<T: Persist> IntoIterator for PersistentVec<T> {
    type Item = T;
    type IntoIter = std::collections::vec_deque::IntoIter<T>;

    fn into_iter(mut self) -> Self::IntoIter {
        let mut vec = VecDeque::with_capacity(self.len());
        while let Some(back) = self.pop() {
            vec.push_front(back);
        }
        vec.into_iter()
    }
}

impl<'a, T: 'a + Persist> IntoIterator for &'a PersistentVec<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_ref().iter()
    }
}

impl<'a, T: 'a + Persist> IntoIterator for &'a mut PersistentVec<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_mut().iter_mut()
    }
}

impl<T: Persist> Extend<T> for PersistentVec<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }
}

/// Gives a split over a persistent vector
#[derive(Debug)]
pub struct Split<'a, T: Persist> {
    src: &'a PersistentVec<T>,
    split_size: usize,
}

impl<'a, T: Persist> Split<'a, T> {
    /// Gets the number of splits.
    pub fn len(&self) -> usize {
        let start = self.src.len() / self.split_size;
        if self.src.len() % self.split_size == 0 {
            start
        } else {
            start + 1
        }
    }

    /// Gets the n-th split of data, if present
    pub fn get(&self, index: usize) -> Option<&[T]> {
        let lower = index * self.split_size;
        let upper = self.src.len().min((index + 1) * self.split_size);

        if lower < self.src.len() {
            Some(&self.src[lower..upper])
        } else {
            None
        }
    }
}

impl<'a, T: Persist> IntoIterator for &'a Split<'a, T> {
    type Item = &'a [T];
    type IntoIter = vec::IntoIter<&'a [T]>;

    fn into_iter(self) -> Self::IntoIter {
        let size = self.len();
        let mut output = vec![];
        for i in 0..size {
            if let Some(slice) = self.get(i) {
                output.push(slice)
            }
        }
        output.into_iter()
    }
}

/// Gives a split over a persistent vector, allowing for access to access to multiple components of the
/// size vector as slices at once
#[derive(Debug)]
pub struct SplitMut<'a, T: Persist> {
    src: UnsafeCell<&'a mut PersistentVec<T>>,
    split_size: usize,
    access: Box<[AtomicIsize]>,
}

impl<'a, T: Persist> SplitMut<'a, T> {
    fn new(src: &'a mut PersistentVec<T>, split_size: usize) -> Self {
        let split_count = Self::split_count(src, split_size);
        let access = std::iter::repeat_with(|| AtomicIsize::new(0))
            .take(split_count)
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            src: UnsafeCell::new(src),
            split_size,
            access,
        }
    }

    /// Gets the number of splits.
    pub fn len(&self) -> usize {
        unsafe {
            let src = &*self.src.get();
            let split_size = self.split_size;
            Self::split_count(src, split_size)
        }
    }

    fn split_count(src: &PersistentVec<T>, split_size: usize) -> usize {
        let start = src.len() / split_size;
        if src.len() % split_size == 0 {
            start
        } else {
            start + 1
        }
    }

    fn get_source(&self) -> &PersistentVec<T> {
        unsafe { &*self.src.get() }
    }

    fn get_source_mut(&self) -> &mut PersistentVec<T> {
        unsafe { &mut *self.src.get() }
    }

    /// Gets the n-th split of data, if present
    pub fn read(&self, index: usize) -> Option<&[T]> {
        let lower = index * self.split_size;
        let upper = self.get_source().len().min((index + 1) * self.split_size);

        if lower < self.get_source().len() {
            match self.access[index].fetch_update(Ordering::SeqCst, Ordering::Relaxed, |v| {
                if v <= 0 {
                    Some(v - 1)
                } else {
                    None
                }
            }) {
                Ok(_) => Some(&self.get_source()[lower..upper]),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    /// Gets write access to n-th split of data, if present and it's currently not already borrowed
    pub fn write(&self, index: usize) -> Option<&mut [T]> {
        let lower = index * self.split_size;
        let upper = self.get_source().len().min((index + 1) * self.split_size);

        if lower < self.get_source().len() {
            if self.access[index]
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::Relaxed)
                .is_ok()
            {
                Some(&mut self.get_source_mut()[lower..upper])
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct Drain<'a, T: Persist> {
    vec: &'a mut PersistentVec<T>,
    start: usize,
    count: usize,
    removed: usize,
}

impl<'a, T: Persist> Drain<'a, T> {
    fn new<R: RangeBounds<usize>>(vec: &'a mut PersistentVec<T>, bounds: R) -> Self {
        let start = match bounds.start_bound() {
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i + 1,
            Bound::Unbounded => 0,
        };
        let end = match bounds.end_bound() {
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i - 1,
            Bound::Unbounded => vec.len() - 1,
        };
        if start > end {
            panic!("start bound can not be greater than end bound")
        } else if end >= vec.len() {
            panic!("end bound must be less than the length of the vector")
        }
        let count = (end + 1) - start;
        Self {
            vec,
            start,
            count,
            removed: 0,
        }
    }
}

impl<'a, T: Persist> Iterator for Drain<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.removed < self.count {
            self.removed += 1;
            Some(self.vec.remove(self.start))
        } else {
            None
        }
    }
}

impl<'a, T: Persist> Drop for Drain<'a, T> {
    fn drop(&mut self) {
        for _ in self {}
    }
}

#[cfg(test)]
mod test {
    use tempfile::tempdir;

    use crate::persist::block::Blocks;

    use super::*;

    #[test]
    fn can_push() {
        let block = Blocks.new();
        let mut p_vec = PersistentVec::new(block);

        p_vec.push('a');
        assert_eq!(p_vec.len(), 1);
        assert_eq!(&p_vec[..], &['a']);

        p_vec.block.hexdump(0);
    }

    #[test]
    fn can_push_file() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("temp#1");

        let block = Blocks.builder().with_size(64).open(file).unwrap();
        let mut p_vec = PersistentVec::new(block);

        for (i, c) in ('a'..='z').into_iter().enumerate() {
            p_vec.push(c);
            assert_eq!(p_vec.len(), i + 1);
            assert_eq!(&p_vec[i], &c);
        }

        p_vec.block.hexdump(0);
    }

    #[test]
    fn can_pop() {
        let block = Blocks.new();
        let mut p_vec = PersistentVec::new(block);

        assert!(matches!(p_vec.pop(), None));
        p_vec.push(14);
        assert_eq!(p_vec.pop(), Some(14));
        assert!(matches!(p_vec.pop(), None));
    }

    #[test]
    fn as_slice() {
        let block = Blocks.new();
        let p_vec = PersistentVec::<usize>::new(block);
        let slice = p_vec.as_ref();
        assert_eq!(slice.len(), 0);
    }

    #[test]
    fn ref_iterate() {
        let mut p_vec = PersistentVec::<usize>::new(Blocks.new());
        p_vec.push(1);
        for &a in &p_vec {
            assert_eq!(a, 1);
        }
    }

    #[test]
    fn iterate() {
        let mut p_vec = PersistentVec::<usize>::new(Blocks.new());
        p_vec.push(1);
        for a in p_vec {
            assert_eq!(a, 1);
        }
    }

    #[test]
    fn remove_arbitrary() {
        let mut p_vec = PersistentVec::with_iter(Blocks.new(), 0..5);
        assert_eq!(p_vec.len(), 5);
        let removed = p_vec.remove(1);
        assert_eq!(removed, 1);
        assert_eq!(p_vec.len(), 4);
        assert_eq!(p_vec[0], 0);
        assert_eq!(p_vec[1], 2);
        assert_eq!(p_vec[2], 3);
        assert_eq!(p_vec[3], 4);
        assert_eq!(p_vec.remove(2), 3);
        assert_eq!(p_vec[2], 4);

        assert_eq!(&p_vec[..], &[0, 2, 4])
    }

    #[test]
    fn can_retain() {
        let mut p_vec = PersistentVec::with_iter(Blocks.new(), 0..5);
        p_vec.retain(|v| v % 2 == 0);
        assert_eq!(&p_vec[..], &[0, 2, 4])
    }

    #[test]
    fn drain_inclusive() {
        let mut p_vec = PersistentVec::with_iter(Blocks.new(), 0..5);
        let drained = p_vec.drain(1..=3).collect::<Vec<_>>();
        assert_eq!(&drained[..], &[1, 2, 3]);
    }

    #[test]
    fn drain_exclusive() {
        let mut p_vec = PersistentVec::with_iter(Blocks.new(), 0..5);
        let drained = p_vec.drain(1..3).collect::<Vec<_>>();
        assert_eq!(&drained[..], &[1, 2]);
    }

    #[test]
    fn drain_full() {
        let mut p_vec = PersistentVec::with_iter(Blocks.new(), 0..5);
        let drained = p_vec.drain(..).collect::<Vec<_>>();
        assert_eq!(&drained[..], &[0, 1, 2, 3, 4]);
    }

    #[test]
    fn vector_is_persisted() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("temp#1");

        {
            let block = Blocks.builder().with_size(512).open(&file).unwrap();
            let p_vec = PersistentVec::<char>::with_iter(block, 'a'..='z');
            assert_eq!(p_vec.len(), 26);
            assert_eq!(p_vec[25], 'z');
        }

        assert!(file.metadata().unwrap().len() > 0);

        {
            let block = Blocks.builder().with_size(512).open(&file).unwrap();
            let p_vec = PersistentVec::<char>::new(block);
            assert_eq!(p_vec.len(), 26);
            assert_eq!(p_vec[25], 'z');
        }
    }

    #[test]
    fn split() {
        let block = Blocks.new();
        let vec: PersistentVec<_> = PersistentVec::with_iter(block, 0..1000);
        let split = vec.split(100);
        assert_eq!(split.len(), 10);

        for (index, split) in split.into_iter().enumerate() {
            println!("split = {split:?}");
            assert_eq!(index * 100, split[0]);
        }
    }

    #[test]
    fn split_mut() {
        let block = Blocks.new();
        let mut vec: PersistentVec<_> = PersistentVec::with_iter(block, 0..100);
        let split = vec.split_mut(50);
        assert_eq!(split.len(), 2);

        let mut lower = split.write(0).unwrap();
        lower[5] = 10;
        let mut upper = split.write(1).unwrap();
        upper[10] = 15;
    }

    #[test]
    fn split_protects_data() {
        let block = Blocks.new();
        let mut vec: PersistentVec<_> = PersistentVec::with_iter(block, 0..100);
        let split = vec.split_mut(50);
        assert_eq!(split.len(), 2);

        {
            let lower_mut = split.write(0).unwrap();
            assert!(split.write(0).is_none());
            assert!(split.read(0).is_none());
        }
        {
            let lower = split.read(0).unwrap();
            assert!(split.write(0).is_none());
            assert!(split.read(0).is_some());
        }
    }
}

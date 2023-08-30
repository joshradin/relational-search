//! A segments is a piece of memory where stuff is stored.
//!
//! Segments store actual data, and can be flushed to/read from disk.

use std::alloc::Layout;
use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter, Pointer};
use std::fs::File;
use std::hash::Hash;
use std::io;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use memmap::MmapMut;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// A marker trait for types that can be persisted
pub trait Persist {}

impl<T: Copy> Persist for T {}

static OPEN_PATHS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

/// A segment key
type BlockKey = u64;

/// Default segment size is 512Kb
pub const DEFAULT_SEGMENT_SIZE: usize = 16;

/// A builder for creating segments
pub struct BlockBuilder<T = ()> {
    disk_path: Option<PathBuf>,
    capacity: Option<usize>,
    _kind: PhantomData<T>,
}

impl<T> BlockBuilder<T> {
    /// Sets where the segment data is stored
    pub fn stored_at(mut self, path: impl AsRef<Path>) -> Self {
        self.disk_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets the capacity of the segment
    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = Some(capacity);
        self
    }

    pub fn with_type<T2: Persist>(self) -> BlockBuilder<T2> {
        let BlockBuilder {
            disk_path,
            capacity,
            ..
        } = self;
        BlockBuilder {
            disk_path,
            capacity,
            _kind: PhantomData::<T2>,
        }
    }
}

impl<T: Persist> BlockBuilder<T> {
    pub fn build(self) -> Result<Block<T>, BlockError> {
        if let Some(ref path) = self.disk_path {
            let used = OPEN_PATHS.get_or_init(|| Default::default());
            let mut guard = used.lock();
            if guard.contains(path) {
                return Err(BlockError::PathAlreadyOpened(path.clone()));
            } else {
                guard.insert(path.to_path_buf());
            }
        }

        let (space_req, capacity) = match self.capacity {
            Some(capacity) => (size_of::<T>(capacity), capacity),
            None => match &self.disk_path {
                None => {
                    return Err(BlockError::MissingCapacity { is_anon: true });
                }
                Some(path) => {
                    if path.exists() {
                        let len = path.metadata()?.len() as usize;
                        let found_cap = len / std::mem::size_of::<T>();
                        (size_of::<T>(found_cap), found_cap)
                    } else {
                        return Err(BlockError::MissingCapacity { is_anon: false });
                    }
                }
            },
        };

        let mmap = match &self.disk_path {
            None => MmapMut::map_anon(space_req)?,
            Some(path) => create_mmap(space_req, path)?,
        };

        let mut segment = Block {
            disk_path: self.disk_path,
            mem_map: mmap,
            capacity,
            _kind: PhantomData,
        };

        Ok(segment)
    }
}

/// Gets the number of bytes needed to store `capacity` number of values `T`.
pub fn size_of<T: Sized>(count: usize) -> usize {
    std::mem::size_of::<T>() * count
}

fn create_mmap(space_req: usize, path: &Path) -> Result<MmapMut, BlockError> {
    let file = File::options()
        .write(true)
        .read(true)
        .create(true)
        .open(path)?;

    if file.metadata()?.len() != space_req as u64 {
        file.set_len(space_req as u64)?;
    }

    let mmap = unsafe { MmapMut::map_mut(&file)? };
    Ok(mmap)
}

/// Fluent api for creating segments
pub struct Blocks;

impl Blocks {
    /// Creates a new, anonymous block with the default value
    pub fn new<T: Persist>(&self) -> Block<T> {
        Self.builder().with_type::<T>().build().unwrap()
    }

    /// Creates a segment builder
    pub fn builder(&self) -> BlockBuilder {
        BlockBuilder {
            disk_path: None,
            capacity: None,
            _kind: PhantomData,
        }
    }
}

/// An error occured creating a block
#[derive(Debug, Error)]
pub enum BlockError {
    #[error("Must specify capacity ({})", if *is_anon { "anonymous map"} else {"file is not present"})]
    MissingCapacity { is_anon: bool },
    #[error("Path {0} already open, only one block can open a file at a time")]
    PathAlreadyOpened(PathBuf),
    #[error(transparent)]
    IoError(#[from] io::Error),
}

/// A segment stores data in memory and with a file backing
pub struct Block<T: Persist> {
    disk_path: Option<PathBuf>,
    mem_map: MmapMut,
    capacity: usize,
    _kind: PhantomData<T>,
}

impl<T: Debug + Persist> Debug for Block<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Segment")
            .field("disk_path", &self.disk_path)
            .field("size", &self.mem_map.as_ref().len())
            .finish_non_exhaustive()
    }
}

impl<T: Persist> Block<T> {
    /// Hex dumps the contents of this segment
    pub fn hexdump(&self, count: usize, page: usize) {
        let start = (count * page).clamp(0, self.size());
        let end = (count * (page + 1)).clamp(0, self.size());
        hexdump::hexdump(&self.mem_map.as_ref()[start..end])
    }

    /// Gets the size of the segment
    pub fn size(&self) -> usize {
        self.mem_map.as_ref().len()
    }

    /// Gets the capacity of the block for holding it's prescribed type
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Gets a pointer to mmap
    pub fn as_ptr(&self) -> *const T {
        self.mem_map.as_ptr() as *const _
    }

    /// Gets a mutable pointer to mmap
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.mem_map.as_mut_ptr() as *mut _
    }

    /// Reads the backing memory as a reference
    pub unsafe fn as_ref(&self) -> &[MaybeUninit<T>] {
        let ptr = self.as_ptr() as *const MaybeUninit<T>;
        &*std::ptr::slice_from_raw_parts(ptr, self.capacity)
    }

    /// Reads the backing memory as a mutable reference
    pub unsafe fn as_mut(&mut self) -> &mut [MaybeUninit<T>] {
        let ptr = self.as_mut_ptr() as *mut MaybeUninit<T>;
        &mut *std::ptr::slice_from_raw_parts_mut(ptr, self.capacity)
    }

    /// Transmutes the type of the block
    pub unsafe fn transmute<U: Persist>(self) -> Block<U> {
        todo!()
    }

    /// reserves an additional amount of space in of disk space, determined by `sizeof(T) * additional` .
    ///
    /// # Panic
    /// panics if the additional amount of space could not be overwritten
    pub unsafe fn reserve(&mut self, additional: usize) {
        let new = size_of::<T>(additional + self.capacity);
        match &self.disk_path {
            None => {
                let mut mmap = MmapMut::map_anon(new).expect("could not create new");
                mmap[..size_of::<T>(self.capacity)].clone_from_slice(self.mem_map.as_ref());
                self.capacity += additional;
                self.mem_map = mmap;
            }
            Some(path) => {
                let mut mmap = create_mmap(new, path).expect("could create new map");
                mmap[..size_of::<T>(self.capacity)].clone_from_slice(self.mem_map.as_ref());
                self.capacity += additional;
                self.mem_map = mmap;
            }
        }

    }
}

impl<T: Persist> Drop for Block<T> {
    fn drop(&mut self) {
        if let Some(ref path) = &self.disk_path {
            let open_paths = OPEN_PATHS.get().expect("will exist by now if path is set");
            let mut guard = open_paths.lock();
            guard.remove(path);
        }
        drop(self.mem_map.flush());
    }
}
#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn can_create_segment() {
        let mut segment = Blocks.new();

        unsafe {
            let mut v_mut = segment.as_mut();
            *v_mut[8].as_mut_ptr() = 5;
        }

        unsafe {
            let v = segment.as_ref();
            assert_eq!(v.len(), 16);
            assert_eq!(v[8].assume_init(), 5);
        }
    }

    #[test]
    fn single_path_ownership() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("temp#1");

        unsafe {
            let segment = Blocks
                .builder()
                .with_type::<usize>()
                .stored_at(&file)
                .build()
                .unwrap();

            Blocks
                .builder()
                .with_type::<usize>()
                .stored_at(&file)
                .build()
                .unwrap_err();
        }
    }

    #[test]
    fn reload_segment() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("temp#1");

        unsafe {
            let mut segment = Blocks
                .builder()
                .with_type::<usize>()
                .with_capacity(1)
                .stored_at(&file)
                .build()
                .unwrap();

            *segment.as_mut()[0].as_mut_ptr() = 15;
        }

        unsafe {
            let mut segment = Blocks
                .builder()
                .with_capacity(1)
                .with_type::<usize>()
                .stored_at(&file)
                .build()
                .unwrap();

            assert_eq!(segment.as_ref()[0].assume_init(), 15);
        }
    }

    #[test]
    fn reserve_anon() {
        let mut block = Blocks
            .builder()
            .with_type::<usize>()
            .with_capacity(1)
            .build()
            .unwrap();
        assert_eq!(block.size(), 8);
        assert_eq!(block.capacity(), 1);

        unsafe {
            block.reserve(1);
        }

        assert_eq!(block.size(), 16);
        assert_eq!(block.capacity(), 2);
    }

    #[test]
    fn reserve_file() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("temp#1");

        let mut block = Blocks
            .builder()
            .with_type::<usize>()
            .with_capacity(1)
            .stored_at(&file)
            .build()
            .unwrap();
        assert_eq!(block.size(), 8);
        assert_eq!(block.capacity(), 1);

        unsafe {
            *block.as_mut_ptr() = 15;
            block.reserve(1);
        }

        assert_eq!(block.size(), 16);
        assert_eq!(block.capacity(), 2);

        unsafe {
            assert_eq!(*block.as_ptr(), 15);
            block.hexdump(512, 0);
        }
    }
}

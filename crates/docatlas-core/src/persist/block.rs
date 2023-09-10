//! A segments is a piece of memory where stuff is stored.
//!
//! Segments store actual data, and can be flushed to/read from disk.

use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter, Pointer};
use std::fs::File;
use std::hash::Hash;
use std::marker::PhantomData;
use std::mem::transmute;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::{io, ptr, slice};

use memmap::MmapMut;
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::persist::Persist;

static OPEN_PATHS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

/// A segment key
type BlockKey = u64;

/// Default segment size is 4096Kb, which is the standard page size
pub const DEFAULT_SEGMENT_SIZE: usize = 1028 * 8 * 4;

/// A builder for creating segments
#[derive(Debug)]
pub struct BlockBuilder {
    size: Option<usize>,
}

impl BlockBuilder {
    /// Sets the size of the block
    pub fn with_size(mut self, size: usize) -> Self {
        self.size = Some(size);
        self
    }

    /// Opens a block at a given path.
    ///
    /// Creates the file at the given path with a set size if the file does not already exist.
    pub fn open<P: AsRef<Path>>(self, path: P) -> Result<Block, BlockError> {
        let path = path.as_ref();

        let mut guard = OPEN_PATHS.get_or_init(Default::default).lock();
        if guard.contains(path) {
            return Err(BlockError::PathAlreadyOpened(path.to_path_buf()));
        } else {
            guard.insert(path.to_path_buf());
        }

        let file = match path.exists() {
            true => File::options().write(true).read(true).open(path)?,
            false => {
                let mut file = File::options()
                    .write(true)
                    .read(true)
                    .create(true)
                    .open(path)?;
                let Some(size) = self.size else {
                    return Err(BlockError::MissingSize { is_anon: false });
                };
                file.set_len(size as u64)?;
                file
            }
        };

        unsafe {
            let map = MmapMut::map_mut(&file)?;
            Ok(Block {
                disk_path: Some(path.to_path_buf()),
                mem_map: map,
            })
        }
    }

    /// Creates a block that's stored anonymously
    pub fn create(self) -> Result<Block, BlockError> {
        match self.size {
            None => Err(BlockError::MissingSize { is_anon: true }),
            Some(size) => Ok(Block {
                disk_path: None,
                mem_map: MmapMut::map_anon(size)?,
            }),
        }
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
    /// Creates a new, anonymous block with a capacity of [`DEFAULT_SEGMENT_SIZE`](DEFAULT_SEGMENT_SIZE)
    ///
    /// # Panic
    /// Can p
    pub fn new(&self) -> Block {
        Self.builder()
            .with_size(DEFAULT_SEGMENT_SIZE)
            .create()
            .unwrap()
    }

    /// Creates a segment builder
    pub fn builder(&self) -> BlockBuilder {
        BlockBuilder { size: None }
    }
}

/// An error occured creating a block
#[derive(Debug, Error)]
pub enum BlockError {
    #[error("Must specify capacity ({})", if *is_anon { "anonymous map"} else {"file is not present"})]
    MissingSize { is_anon: bool },
    #[error("Path {0} already open, only one block can open a file at a time")]
    PathAlreadyOpened(PathBuf),
    #[error(transparent)]
    IoError(#[from] io::Error),
}

/// A segment stores data in memory and with a file backing
pub struct Block {
    disk_path: Option<PathBuf>,
    mem_map: MmapMut,
}

impl Debug for Block {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Segment")
            .field("disk_path", &self.disk_path)
            .field("size", &self.mem_map.as_ref().len())
            .finish_non_exhaustive()
    }
}

impl Block {
    /// Hex dumps the contents of this segment, one page (4096 kb) at time
    pub fn hexdump(&self, page: usize) {
        let count = 1028 * 4 * 8;
        let start = (count * page).clamp(0, self.size());
        let end = (count * (page + 1)).clamp(0, self.size());
        hexdump::hexdump(&self.mem_map.as_ref()[start..end])
    }

    /// Gets the size of the segment
    pub fn size(&self) -> usize {
        self.mem_map.as_ref().len()
    }

    /// Gets a pointer to mmap
    pub unsafe fn as_ptr(&self) -> *const u8 {
        self.mem_map.as_ptr()
    }

    /// Gets a mutable pointer to mmap
    pub unsafe fn as_ptr_mut(&mut self) -> *mut u8 {
        self.mem_map.as_mut_ptr()
    }

    /// Gets an aligned pointer to a type within this block
    pub unsafe fn as_typed_ptr<T: Persist>(&self) -> *const T {
        let ptr = self.as_ptr();
        ptr as *const T
    }

    /// Gets an aligned mutable pointer to a type within this block
    pub unsafe fn as_typed_mut_ptr<T: Persist>(&mut self) -> *mut T {
        let ptr = self.as_ptr_mut();
        ptr as *mut T
    }

    /// reserves an additional amount of bytes of space in of disk space.
    ///
    /// # Panic
    /// panics if the additional amount of space could not be overwritten
    pub unsafe fn reserve(&mut self, additional: usize) {
        let old_size = self.size();
        let new = additional + self.size();
        match &self.disk_path {
            None => {
                let mut mmap = MmapMut::map_anon(new).expect("could not create new");
                mmap[..old_size].clone_from_slice(self.mem_map.as_ref());
                self.mem_map = mmap;
            }
            Some(path) => {
                let mut mmap = create_mmap(new, path).expect("could create new map");
                mmap[..old_size].clone_from_slice(self.mem_map.as_ref());
                self.mem_map = mmap;
            }
        }
    }

    /// Asserts that this block can store a given type
    ///
    /// # Panic
    /// Will panic if this block could not store the given type
    pub fn assert_can_contain<T>(&self) {
        unsafe {
            let offset = self.as_ptr().align_offset(std::mem::align_of::<T>());
            assert!(
                self.size() - offset >= std::mem::size_of::<T>(),
                "size: {}, offset: {}, sizeof<T>: {}",
                self.size(),
                offset,
                std::mem::size_of::<T>()
            )
        }
    }
}

impl Drop for Block {
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
    fn can_create_block() {
        let mut block = Blocks.new();

        unsafe {
            let mut v_mut = block.as_ptr_mut();
            *v_mut = 5;
            block.hexdump(0)
        }
    }

    #[test]
    fn can_use_block_for_arbitrary_type() {
        unsafe {
            let mut block = Blocks.new();

            let mut v_mut = block.as_typed_mut_ptr::<(usize, u32)>();
            *v_mut = (usize::MAX, 0);

            let v_1 = &mut (*v_mut).1;
            *v_1 = 1;

            block.hexdump(0)
        }
    }

    #[test]
    fn single_path_ownership() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("temp#1");

        unsafe {
            let segment = Blocks.builder().open(&file).unwrap();

            Blocks.builder().open(file).unwrap_err();
        }
    }

    #[test]
    fn reload_segment() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("temp#1");

        unsafe {
            let mut segment = Blocks.builder().with_size(1).open(&file).unwrap();

            *segment.as_ptr_mut() = 15;
        }

        unsafe {
            let mut segment = Blocks.builder().with_size(1).open(&file).unwrap();

            assert_eq!(*segment.as_ptr_mut(), 15);
        }
    }

    #[test]
    fn reserve_anon() {
        let mut block = Blocks.builder().with_size(8).create().unwrap();
        assert_eq!(block.size(), 8);

        unsafe {
            block.reserve(8);
        }

        assert_eq!(block.size(), 8);
    }

    #[test]
    fn reserve_file() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("temp#1");

        let mut block = Blocks.builder().with_size(512).open(&file).unwrap();
        assert_eq!(block.size(), 512);

        unsafe {
            *block.as_ptr_mut() = 15;
            block.reserve(512);
        }

        assert_eq!(block.size(), 1028);

        unsafe {
            assert_eq!(*block.as_ptr(), 15);
            block.hexdump(512);
        }
    }
}

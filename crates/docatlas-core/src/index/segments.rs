//! A segments is a piece of memory where stuff is stored.
//!
//! Segments store actual data, and can be flushed to/read from disk.

use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter, Pointer};
use std::fs::File;
use std::io;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use memmap::MmapMut;
use parking_lot::RwLock;
use postcard::ser_flavors::Slice;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::prelude::*;

/// A segment key
type SegmentKey = u64;

/// Default segment size is 512Kb
pub const DEFAULT_SEGMENT_SIZE: usize = 1028 * 512;

/// A builder for creating segments
pub struct SegmentBuilder<T = ()> {
    disk_path: Option<PathBuf>,
    size: usize,
    initial_data: Option<T>,
}

impl<T> SegmentBuilder<T> {

    /// Sets where the segment data is stored
    pub fn stored_at(mut self, path: impl AsRef<Path>) -> Self {
        self.disk_path = Some(path.as_ref().to_path_buf());
        self
    }

    /// Sets the capacity of the segment
    pub fn with_capacity(mut self, size: usize) -> Self {
        self.size = size;
        self
    }

    pub fn with_type<T2: DeserializeOwned + Serialize>(self) -> SegmentBuilder<T2> {
        let SegmentBuilder {
            disk_path, size, ..
        } = self;
        SegmentBuilder {
            disk_path,
            size,
            initial_data: Option::<T2>::None,
        }
    }

    pub fn with_initial_value<T2: DeserializeOwned + Serialize>(
        self,
        value: T2,
    ) -> SegmentBuilder<T2> {
        let SegmentBuilder {
            disk_path, size, ..
        } = self;
        SegmentBuilder {
            disk_path,
            size,
            initial_data: Some(value),
        }
    }

    pub fn with_default_value<T2: DeserializeOwned + Serialize + Default>(
        self,
    ) -> SegmentBuilder<T2> {
        let SegmentBuilder {
            disk_path, size, ..
        } = self;
        SegmentBuilder {
            disk_path,
            size,
            initial_data: Some(T2::default()),
        }
    }
}

impl<T: DeserializeOwned + Serialize> SegmentBuilder<T> {
    pub fn build(self) -> Result<Segment<T>, io::Error> {
        let is_empty = if let Some(path) = &self.disk_path {
            !path.exists()
        } else {
            self.initial_data.is_none()
        };

        if is_empty {
            return Err(io::Error::new(ErrorKind::InvalidData, "must have initial value set for new buffers").into())
        }

        let (file, mut mmap) = match &self.disk_path {
            None => (None, MmapMut::map_anon(self.size)?),
            Some(path) => {
                let file = File::options().write(true).read(true).open(path)?;

                if file.metadata()?.len() != self.size as u64 {
                    file.set_len(self.size as u64)?;
                }

                let mmap = unsafe { MmapMut::map_mut(&file)? };
                (Some(file), mmap)
            }
        };

        if let Some(ref value) = self.initial_data {
            write_to_map(value, &mut mmap).map_err(|e| {
                Error::new(io::Error::new(ErrorKind::InvalidData, "invalid initial data")).with_cause(e)
            })?;
        }

        Ok(Segment {
            file,
            disk_path: self.disk_path,
            mem_map: mmap,
            cached_read_value: RwLock::default(),
        })
    }
}

/// Fluent api for creating segments
pub struct Segments;

impl Segments {
    /// Creates a segment builder
    pub fn builder<'de>(&self) -> SegmentBuilder {
        SegmentBuilder {
            disk_path: None,
            size: DEFAULT_SEGMENT_SIZE,
            initial_data: None,
        }
    }
}

/// A segment
pub struct Segment<T> {
    file: Option<File>,
    disk_path: Option<PathBuf>,
    mem_map: MmapMut,
    cached_read_value: RwLock<Option<Arc<T>>>,
}

impl<T: Debug> Debug for Segment<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Segment")
            .field("disk_path", &self.disk_path)
            .field("size", &self.mem_map.as_ref().len())
            .finish_non_exhaustive()
    }
}

impl<T: DeserializeOwned> Segment<T> {
    pub fn try_get(&self) -> Result<Ref<T>, postcard::Error> {
        {
            let guard = self.cached_read_value.read();
            if let Some(arc) = &*guard {
                return Ok(Ref {
                    data: arc.clone(),
                    _lifetime: PhantomData,
                });
            }
        }

        let mut guard = self.cached_read_value.write();
        let value: T = postcard::from_bytes(self.mem_map.as_ref())?;
        let arc = Arc::new(value);
        *guard = Some(arc.clone());
        return Ok(Ref {
            data: arc,
            _lifetime: PhantomData,
        });
    }

    pub fn try_get_mut(&mut self) -> Result<RefMut<T>, postcard::Error>
    where
        T: Serialize,
    {
        let value: T = postcard::from_bytes(self.mem_map.as_ref())?;

        self.cached_read_value.write().take();

        Ok(RefMut {
            data: value,
            write_accessed: false,
            mmap: &mut self.mem_map,
        })
    }
}

pub struct Ref<'a, T> {
    data: Arc<T>,
    _lifetime: PhantomData<&'a T>,
}

impl<'a, T: Debug> Debug for Ref<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.data, f)
    }
}

impl<'a, T: Display> Display for Ref<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.data, f)
    }
}

impl<'a, T> AsRef<T> for Ref<'a, T> {
    fn as_ref(&self) -> &T {
        &*self.data
    }
}
impl<'a, T> Deref for Ref<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*self.data
    }
}

pub struct RefMut<'a, T: Serialize> {
    data: T,
    write_accessed: bool,
    mmap: &'a mut MmapMut,
}

impl<'a, T: Serialize + Debug> Debug for RefMut<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.data, f)
    }
}

impl<'a, T: Serialize + Display> Display for RefMut<'a, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.data, f)
    }
}

impl<'a, T: Serialize> RefMut<'a, T> {
    /// Flushes the current state of the value to disk
    pub fn flush(&mut self) -> Result<(), postcard::Error> {
        if self.write_accessed {
            write_to_map(&self.data, self.mmap)?;
        }
        Ok(())
    }
}

impl<'a, T: Serialize> AsRef<T> for RefMut<'a, T> {
    fn as_ref(&self) -> &T {
        &self.data
    }
}
impl<'a, T: Serialize> AsMut<T> for RefMut<'a, T> {
    fn as_mut(&mut self) -> &mut T {
        self.write_accessed = true;
        &mut self.data
    }
}

impl<'a, T: Serialize> Deref for RefMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
impl<'a, T: Serialize> DerefMut for RefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.write_accessed = true;
        &mut self.data
    }
}

impl<'a, T: Serialize> Drop for RefMut<'a, T> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

fn write_to_map<T: Serialize>(value: &T, mmap: &mut MmapMut) -> Result<(), postcard::Error> {
    let buffer = mmap.as_mut();
    postcard::serialize_with_flavor(value, Slice::new(buffer))?;
    Ok(())
}

/// Segment holder and creator
#[derive(Debug)]
pub struct SegmentMap<T: Serialize + DeserializeOwned> {
    segments: HashMap<SegmentKey, RwLock<Segment<T>>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_create_segment() {
        let mut segment = Segments
            .builder()
            .with_default_value::<HashMap<usize, usize>>()
            .build()
            .unwrap();

        {
            let mut v_mut = segment.try_get_mut().unwrap();
            v_mut.insert(1, 5);
        }

        let v = segment.try_get().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[&1], 5);
    }

    #[test]
    fn segment_alloc_fail_with_size() {
        let error = Segments
            .builder()
            .with_capacity(4)
            .with_initial_value(0xFFFFFFFF00000000_u64)
            .build()
            .unwrap_err();

        println!("{}", error);
    }
}

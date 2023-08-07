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
use postcard::fixint::be::serialize;
use postcard::ser_flavors::Slice;
use serde::de::DeserializeOwned;
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

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
        let is_empty = self.initial_data.is_none()
            && if let Some(path) = &self.disk_path {
                !path.exists()
            } else {
                true
            };

        if is_empty {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "must have initial value set for new buffers",
            )
            .into());
        }

        let (file, mut mmap) = match &self.disk_path {
            None => (None, MmapMut::map_anon(self.size)?),
            Some(path) => {
                let file = File::options()
                    .write(true)
                    .read(true)
                    .create(true)
                    .open(path)?;

                if file.metadata()?.len() != self.size as u64 {
                    file.set_len(self.size as u64)?;
                }

                let mmap = unsafe { MmapMut::map_mut(&file)? };
                (Some(file), mmap)
            }
        };

        if let Some(ref value) = self.initial_data {
            write_to_map(value, &mut mmap).map_err(|e| {
                Error::new(io::Error::new(
                    ErrorKind::InvalidData,
                    "invalid initial data",
                ))
                .with_cause(e)
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


#[derive(Debug, Serialize, Deserialize)]
struct SegmentPersist<T> {
    disk_path: PathBuf,
    size: usize,
    kind: PhantomData<T>,
}

impl<T> TryFrom<&Segment<T>> for SegmentPersist<T> {
    type Error = SegmentPersistError;

    fn try_from(value: &Segment<T>) -> std::result::Result<Self, Self::Error> {
        match &value.disk_path {
            None => Err(SegmentPersistError::MustBeFileBacked),
            Some(path) => Ok(SegmentPersist {
                disk_path: path.clone(),
                size: value.size(),
                kind: PhantomData,
            }),
        }
    }
}

impl<T> Serialize for Segment<T> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match SegmentPersist::try_from(self) {
            Ok(ok) => ok.serialize(serializer),
            Err(e) => Err(S::Error::custom(e)),
        }
    }
}

impl<T: Serialize + DeserializeOwned> TryFrom<SegmentPersist<T>> for Segment<T> {
    type Error = Error<io::Error>;

    fn try_from(value: SegmentPersist<T>) -> std::result::Result<Self, Self::Error> {
        Segments
            .builder()
            .with_type::<T>()
            .with_capacity(value.size)
            .stored_at(&value.disk_path)
            .build()
    }
}

impl<'de, T: Serialize + DeserializeOwned> Deserialize<'de> for Segment<T> {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let persist = SegmentPersist::<T>::deserialize(deserializer)?;
        Segment::try_from(persist).map_err(|e| serde::de::Error::custom(e))
    }
}

#[derive(Debug, Error)]
pub enum SegmentPersistError {
    #[error("segment must be file backed to be persisted")]
    MustBeFileBacked,
}



/// A segment stores data in memory and with a file backing
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

impl<T> Segment<T> {

    /// Hex dumps the contents of this segment
    pub fn hexdump(&self, count: usize, page: usize) {
        let start = (count * page).clamp(0, self.size());
        let end = (count * (page + 1)).clamp(0, self.size());
        hexdump::hexdump(&self.mem_map.as_ref()[
            start..end
        ])
    }

    /// Gets the size of the segment
    pub fn size(&self) -> usize {
        self.mem_map.as_ref().len()
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
    use tempfile::tempdir;

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

    #[test]
    fn multi_layer_segments() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("temp#1");

        let map = (0..64).into_iter().map(|i| (i, i*i)).collect::<HashMap<_, _>>();

        let mut inner = Segments
            .builder()
            .stored_at(file)
            .with_capacity(512)
            .with_initial_value(map)
            .build()
            .unwrap();

        inner.hexdump(256, 0);


        let segment = Segments
            .builder()
            .with_initial_value([inner])
            .build()
            .unwrap();
    }
}

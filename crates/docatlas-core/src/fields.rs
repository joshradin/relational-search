//! The fields that make up a document.

use num_bigfloat::BigFloat;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{Arc, RwLock};

/// A view of a set of fields.
#[derive(Debug)]
pub struct Fields {
    map: HashMap<String, Field>,
}

impl<S: AsRef<str>> FromIterator<(S, Field)> for Fields {
    fn from_iter<T: IntoIterator<Item = (S, Field)>>(iter: T) -> Self {
        let map = iter
            .into_iter()
            .map(|(field, value)| (field.as_ref().to_string(), value))
            .collect::<HashMap<_, _>>();
        Self { map }
    }
}

/// A field contains a kind and related data.
///
/// Data is stored non-normally.
#[derive(Debug)]
pub struct Field {
    kind: FieldKind,
    data: Vec<FieldData>,
}

/// The kind of the field
#[derive(Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// Keywords are non-tokenized
    Keyword,
    /// Text fields are tokenized
    Text,
    /// Just a number
    Number,
    /// A sub document
    Nested,
}

impl FieldKind {
    /// A field kind that is directly indexable.
    pub fn indexable(&self) -> bool {
        todo!()
    }

    /// A field that can be searched, but is not indexed by itself.
    pub fn searchable(&self) -> bool {
        todo!()
    }

    /// A field that can be aggregated against. These are mostly just numbers, aka have
    /// closure over mathematical operations.
    pub fn aggregateable(&self) -> bool {
        todo!()
    }
}

/// A field within a document
///
/// These values are the "raw" values, and are what indexes are built on top of. The Field Kind is used to interpret
/// how the data is actually viewed.
///
/// Field values should be optimized for multiple reading.
#[derive(Debug, PartialEq)]
pub enum FieldData {
    /// The sizet data type, used mainly for identifiers
    SizeT(usize),
    /// Bytes of a set size
    Bytes(Arc<[u8]>),
    /// A floating point number of a dynamic size
    Number(BigFloat),
}

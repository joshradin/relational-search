//! The fields that make up a document.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;

use num_bigfloat::BigFloat;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldKind {
    /// Keywords are non-tokenized
    Keyword(usize),
    /// Text fields are tokenized
    Text(usize),
    /// Just a number
    Number(usize),
}

impl FieldKind {
    /// Gets the size required to store the field
    pub fn size(&self) -> usize {
        match self {
            FieldKind::Keyword(u) => *u,
            FieldKind::Text(u) => *u,
            FieldKind::Number(u) => *u,
        }
    }

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

/// A type that can be represent fields
pub trait AsFields {
    type IntoIter<'a>: IntoIterator<Item = (&'a str, &'a Field)>
    where
        Self: 'a;

    /// Gets the fields of some value
    fn as_fields(&self) -> Self::IntoIter<'_>;
}

impl AsFields for Fields {
    type IntoIter<'a> = Vec<(&'a str, &'a Field)>;

    fn as_fields(&self) -> Self::IntoIter<'_> {
        self.map.iter().map(|(k, v)| (&**k, v)).collect()
    }
}

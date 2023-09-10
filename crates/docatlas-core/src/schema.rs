//! A schema defines the mapping of an index

use crate::fields::FieldKind;
use std::iter::FusedIterator;
use std::ops::{
    Index, Range, RangeBounds, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive,
};
use std::vec::Drain;

/// A schema defines an ordered array of fields
#[derive(Debug)]
pub struct Schema {
    fields: Vec<SchemaField>,
}

impl Schema {
    /// Creates a new, empty schema
    pub fn new() -> Self {
        Self { fields: vec![] }
    }

    /// Gets an iterator over the schema fields
    pub fn iter(&self) -> SchemaIter {
        SchemaIter {
            members: &self.fields,
            index: 0,
        }
    }

    /// Drains fields in a range
    pub fn drain<R: RangeBounds<usize>>(&mut self, bounds: R) -> Drain<SchemaField> {
        self.fields.drain(bounds)
    }

    /// Get the field at a given index
    pub fn get_at(&self, index: usize) -> Option<&SchemaField> {
        self.fields.get(index)
    }

    /// Get a mutable reference to a  field at a given index
    pub fn get_at_mut(&mut self, index: usize) -> Option<&mut SchemaField> {
        self.fields.get_mut(index)
    }

    /// Gets the first schema field by name, if present
    pub fn get(&self, name: impl AsRef<str>) -> Option<&SchemaField> {
        self.iter().find(|f| &f.name == name.as_ref())
    }

    /// Gets a mutable reference to the first schema field by name, if present
    pub fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut SchemaField> {
        self.fields.iter_mut().find(|f| &f.name == name.as_ref())
    }

    /// Gets the number of bytes required to the store a row of the given schema.
    pub fn row_size(&self) -> usize {
        self.iter().map(|field| field.kind.size()).sum()
    }
}

impl Index<usize> for Schema {
    type Output = SchemaField;

    fn index(&self, index: usize) -> &Self::Output {
        self.get_at(index).unwrap()
    }
}

impl Index<&str> for Schema {
    type Output = SchemaField;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).unwrap()
    }
}

macro_rules! index_range {
    ($($range_type:ty),+) => {
        $(
        impl Index<$range_type> for Schema {
            type Output = [SchemaField];

            fn index(&self, index: $range_type) -> &Self::Output {
                &self.fields[index]
            }
        }
        )*
    };
}

index_range!(
    RangeFull,
    Range<usize>,
    RangeInclusive<usize>,
    RangeTo<usize>,
    RangeToInclusive<usize>,
    RangeFrom<usize>
);

impl Extend<SchemaField> for Schema {
    fn extend<T: IntoIterator<Item = SchemaField>>(&mut self, iter: T) {
        self.fields.extend(iter.into_iter())
    }
}

impl<'a> IntoIterator for &'a Schema {
    type Item = &'a SchemaField;
    type IntoIter = SchemaIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for Schema {
    type Item = SchemaField;
    type IntoIter = <Vec<SchemaField> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

impl FromIterator<SchemaField> for Schema {
    fn from_iter<T: IntoIterator<Item = SchemaField>>(iter: T) -> Self {
        Self {
            fields: iter.into_iter().collect(),
        }
    }
}
#[derive(Debug)]
pub struct SchemaIter<'a> {
    members: &'a [SchemaField],
    index: usize,
}

impl<'a> Iterator for SchemaIter<'a> {
    type Item = &'a SchemaField;

    fn next(&mut self) -> Option<Self::Item> {
        let out = self.members.get(self.index);
        if out.is_some() {
            self.index += 1;
        }
        out
    }
}

impl FusedIterator for SchemaIter<'_> {}

/// A single field in a schema
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaField {
    pub name: String,
    pub kind: FieldKind,
}

#[cfg(test)]
mod tests {
    use crate::schema::Schema;

    #[test]
    fn index_schema() {
        let schema = Schema::new();
        assert_eq!(schema[..].len(), 0);
    }
}

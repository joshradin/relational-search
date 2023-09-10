//! A document is a non-normalized tuple of data that allows for nested types

use serde::Serialize;
use thiserror::Error;

use crate::fields::Fields;
use crate::schema::Schema;

/// A document is made of fields
#[derive(Debug)]
pub struct Document {
    fields: Fields,
}

impl From<Fields> for Document {
    fn from(value: Fields) -> Self {
        Self { fields: value }
    }
}

/// Some document data
#[derive(Debug)]
pub struct DocumentData<'a> {
    data: &'a [u8],
}

impl DocumentData<'_> {
    /// Tries to convert raw data into a human friendly document
    pub fn try_into_document(&self, schema: &Schema) -> Result<Document, DocumentDataError> {
        if schema.row_size() != self.data.len() {
            return Err(DocumentDataError::IncorrectSize {
                expected: schema.row_size(),
                found: self.data.len(),
            });
        }

        todo!()
    }
}

/// An error trying to read document data into a document
#[derive(Debug, Error)]
pub enum DocumentDataError {
    #[error("Incorrect row size (expected: {expected}, found: {found})")]
    IncorrectSize { expected: usize, found: usize },
}

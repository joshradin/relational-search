use std::path::PathBuf;
use crate::fields::Fields;
use crate::index::segments::Segment;

pub mod segments;

#[derive(Debug)]
pub struct Index {
    path: PathBuf,
    documents: Segment<(Vec<Fields>,)>
}
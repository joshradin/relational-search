use crate::fields::Fields;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Index {
    path: PathBuf,
    documents: Vec<(Vec<Fields>,)>,
}

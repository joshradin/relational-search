use std::path::PathBuf;
use crate::fields::Fields;


#[derive(Debug)]
pub struct Index {
    path: PathBuf,
    documents: Vec<(Vec<Fields>,)>
}
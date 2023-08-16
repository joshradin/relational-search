//! Identifiers system. Works similarly to a filesystem, but with rust values.

use crate::shared::{Shared, SharedReadGuard, SharedWriteGuard};
use crate::users::{Group, User, UserContext};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};

use crate::prelude::*;

#[derive(Debug)]
pub struct IdSystem {
    next_id: u64,
    child_to_parent: HashMap<u64, u64>,
    parent_to_children: HashMap<u64, HashSet<u64>>,
    id_nodes: Vec<Option<IdNode>>,
}

impl IdSystem {

    /// Creates a new id system
    pub fn new() -> Self {
        Self {
            next_id: 0,
            child_to_parent: Default::default(),
            parent_to_children: Default::default(),
            id_nodes: vec![],
        }
    }

    /// creates a new id node.
    ///
    /// # Panic
    /// panics if no more id nodes can be created.
    fn new_id_node(
        &mut self,
        name: String,
        user: Shared<User>,
        group: Shared<Group>,
        val: Shared<Box<dyn Any>>,
    ) -> IdNode {
        if name.is_empty() {
            panic!("name can not be empty")
        }
        let id = self.next_id();
        IdNode {
            unique_id: id,
            name,
            user,
            group,
            val,
        }
    }

    /// Gets the next id
    fn next_id(&mut self) -> u64 {
        let next = self.next_id;
        self.next_id += 1;
        if self.next_id != self.id_nodes.len() as u64 {
            while matches!(self.id_nodes.get(next as usize), Some(Some(_))) {
                self.next_id += 1;
            }
        }
        next
    }

    fn get_id_node(&self, id: &RawId) -> Option<&IdNode> {
        match self.id_nodes.get(id.id as usize) {
            None => {
                None
            }
            Some(found) => {
                found.as_ref()
            }
        }
    }

    /// Opens in read mode
    pub fn open<T>(&self, id: &Id, ctx: &UserContext) -> Result<SharedReadGuard<T>, Error> {
        todo!()
    }

    /// Opens in write mode
    pub fn create<T>(&mut self, id: &Id, ctx: &UserContext) -> Result<SharedWriteGuard<T>, Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct Object<'a, T> {
    kind: GuardKind<'a, T>
}

#[derive(Debug)]
enum GuardKind<'a, T> {
    Read(SharedReadGuard<'a, T>),
    Write(SharedWriteGuard<'a, T>)
}

/// An id error
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Permission denied")]
    PermissionDenied
}

/// A node within the id system.
///
/// Contains the actual value.
#[derive(Debug)]
struct IdNode {
    unique_id: u64,
    name: String,
    user: Shared<User>,
    group: Shared<Group>,
    val: Shared<Box<dyn Any>>,
}

impl IdNode {

    /// Gets the raw id for an id node
    fn raw_id(&self) -> RawId {
        RawId { id: self.unique_id }
    }
}

/// An identifier
#[derive(Debug, Clone)]
pub struct Id {
    is_abs: bool,
    disp: Vec<String>,
    raw: RawId
}

impl Display for Id {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_abs {
            write!(f, ":")?;
        }
        write!(f, "{}", self.disp.join(":"))
    }
}

impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        self.raw.eq(&other.raw)
    }
}

impl Eq for Id { }

impl Hash for Id {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state)
    }
}


#[derive(Debug, Eq, PartialEq, Hash, Clone)]
struct RawId {
    id: u64
}

impl RawId {
    fn resolve<'a>(&self, system: &'a IdSystem) -> Option<&'a IdNode>{
        system.get_id_node(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::identifiers::IdSystem;

    #[test]
    fn create_in_root() {
        let is = IdSystem::new();

    }
}
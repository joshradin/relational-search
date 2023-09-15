//! The core of doctatlas.
//!
//!

pub mod auth;
pub mod document;
pub mod fields;
pub mod index;
pub mod persist;
pub mod schema;
pub mod shared;
pub mod transport;

pub mod prelude {
    //! The prelude re-exports common types and functions
    pub use super::persist::*;
}

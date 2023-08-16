//! The core of doctatlas.
//!
//!

pub mod document;
pub mod fields;
pub mod index;
pub mod mem;
pub mod schema;
pub mod shared;
pub mod auth;

pub mod prelude {
    //! The prelude re-exports common types and functions

    pub use crate::{
        auth::lock_api::{Key, Tumbler, Lock}
    };
}

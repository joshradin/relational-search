//! The core of doctatlas.
//!
//!

pub mod document;
pub mod error;
pub mod fields;
pub mod index;

pub mod result {
    //! Contains the result type definition, with the custom [Error](crate::error::Error) as it's result
    use crate::error::Error;

    /// The result type override.
    pub type Result<T, E> = std::result::Result<T, Error<E>>;
}

pub mod prelude {
    //! The prelude re-exports common types and functions

    pub use crate::{
        error::{AnyError, Error},
        result::Result,
    };
}

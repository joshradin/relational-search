//! General error handling, along with caused-by and a backtrace

use std::any::Any;
use std::backtrace::Backtrace;
use std::fmt::{Display, Formatter};
use std::process::{ExitCode, Termination};

/// A more advanced error reporting mechanism, that stores the backtrace and an optional
/// caused by error.
#[derive(Debug)]
pub struct Error<E: ?Sized> {
    error: ErrorStorage<E>,
    backtrace: Backtrace,
    caused_by: Option<Box<AnyError>>,
}

impl<E: ?Sized> Error<E> {
    /// Sets the backtrace for this error.
    pub fn with_backtrace(mut self, backtrace: Backtrace) -> Self {
        self.backtrace = backtrace;
        self
    }

    /// Sets the cause of this error
    pub fn with_cause<E2: std::error::Error + 'static>(mut self, by: Error<E2>) -> Self {
        self.caused_by = Some(Box::new(by.to_any_error()));
        self
    }

    /// Gets the actual error of this error
    pub fn error(&self) -> &E {
        match &self.error {
            ErrorStorage::Raw(e) => e,
            ErrorStorage::WithMessage(e, _) => e,
        }
    }

    /// Gets the backtrace assigned to this error.
    pub fn backtrace(&self) -> &Backtrace {
        &self.backtrace
    }
    /// Gets the caused by error of this error.
    pub fn caused_by(&self) -> Option<&AnyError> {
        self.caused_by.as_ref().map(|b| &**b)
    }
}

impl<E: std::error::Error> Error<E> {
    /// Creates a new error. Backtrace is determined from this call by default.
    #[track_caller]
    pub fn new(err: E) -> Self {
        Self {
            error: ErrorStorage::Raw(Box::new(err)),
            backtrace: Backtrace::capture(),
            caused_by: None,
        }
    }

    /// Converts this error to an any error.
    pub fn to_any_error(self) -> AnyError
    where
        E: 'static,
    {
        let Error {
            error,
            backtrace,
            caused_by,
        } = self;
        let (error, msg) = match error {
            ErrorStorage::Raw(e) => {
                let msg = e.to_string();
                (e, msg)
            }
            ErrorStorage::WithMessage(e, msg) => (e, msg.clone()),
        };
        let as_any = error as Box<dyn Any>;
        AnyError {
            error: ErrorStorage::WithMessage(as_any, msg),
            backtrace,
            caused_by,
        }
    }
}

impl<E: std::error::Error> From<E> for Error<E> {
    fn from(value: E) -> Self {
        Error::new(value)
    }
}

impl AnyError {
    /// Attempts to downcast this any error to a concrete type.
    fn try_downcast<E: std::error::Error + Any>(self) -> Result<Error<E>, Self> {
        let Error {
            error,
            backtrace,
            caused_by,
        } = self;
        let (error, msg) = match error {
            ErrorStorage::Raw(_) => {
                unreachable!("should always have message stored with any error")
            }
            ErrorStorage::WithMessage(e, msg) => (e, msg),
        };
        match error.downcast::<E>() {
            Ok(downcasted) => Ok(Error {
                error: ErrorStorage::Raw(downcasted),
                backtrace,
                caused_by,
            }),
            Err(e) => Err(Error {
                error: ErrorStorage::WithMessage(e, msg),
                backtrace,
                caused_by,
            }),
        }
    }
}

impl<E: Display + std::error::Error + 'static> Display for Error<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.error)?;
        write!(f, "{}", self.backtrace)?;
        if let Some(c) = &self.caused_by {
            write!(f, "caused by: {}", c)?;
        }
        Ok(())
    }
}

impl Display for AnyError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.error)?;
        writeln!(f, "{}", self.backtrace)?;
        if let Some(c) = &self.caused_by {
            write!(f, "caused by: {}", c)?;
        }
        Ok(())
    }
}

/// Dynamic error
pub type AnyError = Error<dyn Any>;

#[derive(Debug)]
enum ErrorStorage<E: ?Sized> {
    Raw(Box<E>),
    WithMessage(Box<E>, String),
}

impl<E: Display + std::error::Error + 'static> Display for ErrorStorage<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            ErrorStorage::Raw(e) => {
                write!(f, "{}", e)
            }
            ErrorStorage::WithMessage(_, msg) => {
                write!(f, "{}", msg)
            }
        }
    }
}

impl Display for ErrorStorage<dyn Any> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            ErrorStorage::Raw(_) => {
                unreachable!()
            }
            ErrorStorage::WithMessage(_, msg) => {
                write!(f, "{}", msg)
            }
        }
    }
}

impl<E> Termination for Error<E> {
    fn report(self) -> ExitCode {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use crate::error::Error;
    use std::io::ErrorKind;

    #[test]
    fn can_do_dynamic() {
        let arb_error = std::io::Error::new(ErrorKind::InvalidData, "invalid data!");
        let error = Error::new(arb_error);
        let as_any = error.to_any_error();
        println!("err = {as_any}");
        assert!(matches!(as_any.try_downcast::<std::io::Error>(), Ok(_)));
    }
}

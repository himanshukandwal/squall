use std::fmt;

/// Errors returned by [`Store`](crate::Store) commands.
///
/// A command that returns an error has made no change to the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Error {
    /// The key holds a value of a different type than the command expects.
    WrongType,
    /// The string value is not a decimal `i64`.
    NotAnInteger,
    /// The arithmetic result does not fit in an `i64`.
    Overflow,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WrongType => {
                f.write_str("operation against a key holding the wrong kind of value")
            }
            Error::NotAnInteger => f.write_str("value is not an integer"),
            Error::Overflow => f.write_str("increment or decrement would overflow"),
        }
    }
}

impl std::error::Error for Error {}

//! Error values.

use core::result;
use core::fmt;

/// Error values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Hash)]
pub enum Error {
    /// Not enough memory to perform the operation.
    OutOfMemory,
    /// Operation could not be completed because an "end of file" was reached prematurely.
    UnexpectedEof,
    /// The system has a library version that is too low.
    UnsupportedLibraryVersion,
    /// To be replaced by core::io::Error.
    ReadError,
    /// To be replaced by core::io::Error.
    WriteError,
    /// To be replaced by core::io::Error.
    SeekError,
    /// File path is too long. The maximum length is 255 characters.
    PathTooLong,
    /// Amiga IoErr value from an I/O routine.
    IoErr(i32)
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::OutOfMemory => write!(f, "out of memory"),
            Error::UnexpectedEof => write!(f, "unexpected eof"),
            Error::UnsupportedLibraryVersion => write!(f, "unsupported library version"),
            Error::ReadError => write!(f, "read error"),
            Error::WriteError => write!(f, "write error"),
            Error::SeekError => write!(f, "seek error"),
            Error::PathTooLong => write!(f, "path value too long"),
            Error::IoErr(val) => write!(f, "error code: {}", val),
        }
    }
}

impl core::error::Error for Error {}

/// Library Result type.
pub(crate) type Result<T> = result::Result<T, Error>;

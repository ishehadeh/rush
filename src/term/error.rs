use failure;
use lang::exec;
use std::os::unix::io::RawFd;
use std::{fmt, result};

pub type Result<T> = result::Result<T, Error>;
#[derive(Debug)]
pub struct Error {
    inner: failure::Context<ErrorKind>,
}

#[derive(Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "failed to put the terminal in raw mode")]
    InitRawModeFailed,

    #[fail(display = "failed to take terminal out of raw mode")]
    ExitRawModeFailed,

    #[fail(display = "failed to get the next character")]
    GetCharFailed,
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }
}

impl failure::Fail for Error {
    fn cause(&self) -> Option<&failure::Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&failure::Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: failure::Context::new(kind),
        }
    }
}

impl From<failure::Context<ErrorKind>> for Error {
    fn from(inner: failure::Context<ErrorKind>) -> Error {
        Error { inner: inner }
    }
}

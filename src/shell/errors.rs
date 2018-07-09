use failure;
use std::{fmt, result};

pub type Result<T> = result::Result<T, Error>;
#[derive(Debug)]
pub struct Error {
    inner: failure::Context<ErrorKind>,
}

#[derive(Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "failed to parse expression")]
    ExpressionParsingError,

    #[fail(display = "system error")]
    SysError,

    #[fail(
        display = "could not find \"{}\" in any paths listed in the $PATH environment variable", _0
    )]
    MissingExecutable(String),

    #[fail(display = "illegal NULL byte in input")]
    IllegalNullByte,

    #[fail(display = "illegal executable name input")]
    IllegalExecutableName,

    #[fail(display = "failed to wait for child process")]
    WaitFailed,

    #[fail(display = "failed to execute child process")]
    ExecFailed,
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

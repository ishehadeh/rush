use failure;
use nom;
use std::fmt;
use std::result;
#[derive(Clone, Debug, Eq, PartialEq, Fail)]
pub enum ErrorKind {
    #[fail(display = "The file is not a valid terminfo file")]
    TerminfoInvalid,

    #[fail(display = "terminfo ended unexpectedly while parsing boolean fields")]
    TerminfoMissingBoolFields,

    #[fail(display = "terminfo ended unexpectedly while parsing numeric fields")]
    TerminfoMissingNumberFields,

    #[fail(display = "terminfo ended unexpectedly while parsing string fields")]
    TerminfoMissingStringFields,

    #[fail(display = "terminfo string table ended unexpectedly")]
    TerminfoMissingStringTableEntries,

    #[fail(display = "Failed to find a null terminator on the terminfo names section")]
    TerminfoUnterminatedNames,

    #[fail(display = "terminfo file is incomplete, expecting at least {:?} more bytes.", _0)]
    TerminfoIncomplete(nom::Needed),

    #[fail(display = "Nom parser failed with error {:?}", _0)]
    TerminfoBadFile(nom::ErrorKind<u32>),

    #[fail(display = "Failed to read terminfo data")]
    IoError,
}

#[derive(Debug)]
pub struct Error {
    inner: failure::Context<ErrorKind>,
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }
}

pub type Result<T> = result::Result<T, Error>;

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

impl From<nom::ErrorKind> for ErrorKind {
    fn from(kind: nom::ErrorKind) -> ErrorKind {
        match kind {
            nom::ErrorKind::Custom(code) => match code {
                1 => ErrorKind::NotATermInfoFile,
                2 => ErrorKind::FailedToFindEndOfNames,
                3 => ErrorKind::TooManyFieldsAtBool,
                4 => ErrorKind::TooManyFieldsAtNumber,
                5 => ErrorKind::TooManyFieldsAtString,
                6 => ErrorKind::EmptyStringTable,
                _ => ErrorKind::NomErr(nom::ErrorKind::Custom(code)),
            },
            _ => ErrorKind::NomErr(kind),
        }
    }
}

impl From<failure::Context<ErrorKind>> for Error {
    fn from(inner: failure::Context<ErrorKind>) -> Error {
        Error { inner: inner }
    }
}

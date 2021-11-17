use failure;
use std::{fmt, result};

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    parser_context: Option<Context>,
    inner: failure::Context<ErrorKind>,
}

#[derive(Debug)]
pub struct Context {
    pub input: String,
    pub token: String,
    pub column: usize,
    pub line: usize,
}

#[derive(Eq, PartialEq, Debug, Fail)]
pub enum ErrorKind {
    #[fail(display = "unable to parse string, illegal character '{}'", _0)]
    InvalidCharacter(char),

    #[fail(display = "invalid token")]
    InvalidToken,

    #[fail(
        display = "unexpected prefix operator, Expecting one of ~, !, +, -, ++, --, a number, or a variable."
    )]
    InvalidPrefixOperator,

    #[fail(display = "unexpected infix operator, expecting an operator like +, -, *, %, etc.")]
    InvalidInfixOperator,

    #[fail(display = "expecting a ternary condition 'else' block beginning with ':'")]
    ExpectingTernaryElse,

    #[fail(display = "expecting right parentheses")]
    ExpectingRightParentheses,

    #[fail(
        display = "invalid number, please only use numbers, unary +/-, decimal points, and exponents."
    )]
    InvalidNumber,

    #[fail(display = "unexpected end-of-expression")]
    UnexpectedEof,
}

impl Error {
    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }

    pub fn with(mut self, ctx: Context) -> Self {
        self.parser_context = Some(ctx);
        self
    }
}

impl failure::Fail for Error {
    fn cause(&self) -> Option<&dyn failure::Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&failure::Backtrace> {
        self.inner.backtrace()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}", self.inner)?;
        match &self.parser_context {
            Some(v) => write!(f, "{}", v),
            None => Ok(()),
        }
    }
}

impl From<failure::Context<ErrorKind>> for Error {
    fn from(inner: failure::Context<ErrorKind>) -> Error {
        Error {
            parser_context: None,
            inner: inner,
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(inner: ErrorKind) -> Error {
        Error {
            parser_context: None,
            inner: failure::Context::new(inner),
        }
    }
}

impl fmt::Display for Context {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let prefix = format!("{} |", " ".repeat(self.line.to_string().len()));

        writeln!(f, "{}", prefix)?;
        writeln!(f, "{} |  {}", self.line, self.input)?;
        writeln!(
            f,
            "{}  {}{}",
            prefix,
            " ".repeat(self.column),
            "^".repeat(self.token.len())
        )
    }
}

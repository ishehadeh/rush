pub mod ast;
mod errors;
pub mod exec;
pub mod parser;
pub mod word;
pub use self::errors::*;
pub use self::exec::ExecutionEnvironment;
use nom;

pub fn parse<'a>(s: &'a str) -> ast::Command<'a> {
    parser::commandline(nom::types::CompleteStr(s)).unwrap().1
}

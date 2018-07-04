pub mod expr;
/// Nom combinators for parsing RUSH shell scripts
///
mod types;

pub use self::types::*;

use nom::types::CompleteStr;
use std::os::unix::io::RawFd;
use std::str::FromStr;

use nom::Needed;

/// eat any string of valid non-newline whitespace characters
/// Characters Recognized as Whitespace:
/// - \t (tab)
/// - \  (space)
named!(pub space<CompleteStr, CompleteStr>, eat_separator!(" \t"));

/// Line endings are whitespace that move the cursor to the next line
named!(
    pub line_ending<CompleteStr, CompleteStr>,
    alt!( tag!("\n") | tag!("\n\r") )
);

/// skip anything chomp-able by `space`
#[macro_export]
macro_rules! sp (
  ($i:expr, $($args:tt)*) => (
    {
      sep!($i, space, $($args)*)
    }
  )
);

/// A conditional can join two commands, depending on the result of the prior command the second may not be executed
///
/// Supported conditional operators
/// - && - proceed only if the last command was successful
/// - || - proceed only if the last command failed
named!(
    pub conditional_operator<CompleteStr, ConditionOperator>,
    alt!(
          tag!("&&")  => { |_| ConditionOperator::AndIf }
        | tag!("||")  => { |_| ConditionOperator::OrIf }
    )
);

/// IO Operators can modify the modify the file descriptor table, close, open, read, and write to files.
///
/// IO operations appear after a command, they are optionally proceeded by a file descriptor, and may be followed by several different types of WORD
///
/// Supported IO Operators:
/// - <<- - Here document (remove tabs)
/// - << - Here document
/// - >> - Append to a file
/// - >| - Open a file for writing (fail if the file does not exist)
/// - >  - Open/Create a file for writing
/// - <  - Open a file for reading
/// - >& - Duplicate an output file descriptor (basically just the dup2 systemcall)
/// - <& - Duplicate an input file descriptor (basically just the dup2 systemcall)
/// - <> - Open a file for reading an writing
named!(
    pub io_operator<CompleteStr, IoOperation>,
    alt!(
          tag!("<<-") => { |_| IoOperation::HereDocumentStrip }
        | tag!(">>")  => { |_| IoOperation::OutputAppend }
        | tag!("<<")  => { |_| IoOperation::HereDocument }
        | tag!(">|")  => { |_| IoOperation::Output }
        | tag!("<&")  => { |_| IoOperation::InputDupFd }
        | tag!(">&")  => { |_| IoOperation::OutputDupFd }
        | tag!("<>")  => { |_| IoOperation::ReadWrite }
        | tag!("<")   => { |_| IoOperation::Input }
        | tag!(">")   => { |_| IoOperation::OutputCreate }
    )
);

/// Pipes connect a command's standard out to another command's standard in
named!(
    pub pipe<CompleteStr, char>,
    one_of!("|")
);

/// A separator splits WORDS into commands when they are on the same line
///
/// For example `echo hello; echo hi` prints "hello<newline>hi"
/// while `echo hello echo hi` prints "hello echo hi"
/// Supported Separators
/// - ; Acts a newline
/// - & same as `;`, but asynchronously calls the previous command
named!(
    pub separator<CompleteStr, Separator>,
    alt!(
          tag!("&")   => { |_| Separator::Fork }
        | tag!(";")   => { |_| Separator::Stop }
    )
);

/// An io number is the file descriptor that comes before the an IO operator
named!(
    pub io_number<CompleteStr, CompleteStr>,
    terminated!(take_while1!(|c| c >= '0' && c <= '9'), one_of!("<>"))
);

named!(
    pub unquoted_string<CompleteStr, CompleteStr>,
    preceded!(not!(io_number), escaped!(is_not!(" \\'\"()|&;<>\t\n"), '\\', one_of!(" \\'\"()|&;<>\t\n~")))
);

named!(
    pub single_quoted_string<CompleteStr, CompleteStr>,
    delimited!(char!('\''), take_until!("\'"), char!('\''))
);

named!(
    pub double_quoted_string<CompleteStr, CompleteStr>,
    delimited!(char!('"'), escaped!(is_not!("\"n\\"), '\\', one_of!("\"n\\")), char!('"'))
);

/// A word is a basic string in a shell script
///
/// Words may be bare, single quoted, and double quoted, or any combination of the three.
/// for example `hello"world "'goodbye'` is a valid word, "helloworld goodbye".
named!(
    pub word<CompleteStr, String>,
    map!(
        recognize!(
            alt!(
                  single_quoted_string
                | double_quoted_string
                | unquoted_string
            )
        ),
        |v| v.to_string()
    )
);

named!(
    pub simple_command<CompleteStr, Command>,
    do_parse!(
        args: separated_list!(space, word) >>
        (Command::simple(&args))
    )
);

named!(
    pub redirect_destination<CompleteStr, RedirectDestination>,
    do_parse!(
        number : opt!(map!(take_while1!(|c| c >= '0' && c <= '9'), |nums| RawFd::from_str(nums.0).unwrap())) >>
        operation : call!(io_operator) >>
        file: opt!(word) >>
        (RedirectDestination::new(operation, number, file))
    )
);

named!(
    pub redirect<CompleteStr, Command>,
    do_parse!(
        command  : sp!(simple_command) >>
        redirect : opt!(many1!(sp!(redirect_destination))) >>
        (if redirect.is_none() {
            command
        } else {
            Command::redirect(command, &redirect.unwrap())
        })
    )
);

named!(
    pub pipeline<CompleteStr, Command>,
    do_parse! (
        bang: opt!(sp!(tag!("!"))) >>
        initial : sp!(redirect) >>
        sub: fold_many0!(
            do_parse!(
                _op: sp!(pipe) >>
                expr: sp!(redirect) >>
                (expr)
            ),
            initial,
            |start, expr| {
                Command::pipeline(bang.is_some(), start, expr)
            }
        ) >> (sub)
    )
);

named!(
    pub list<CompleteStr, Command>,
    do_parse! (
        initial:  sp!(pipeline) >>
        extended: fold_many0!(
            do_parse!(
                op   : sp!(conditional_operator) >>
                expr : sp!(pipeline) >>
                (op, expr)
            ),
            initial,
            |start: Command, (op, expr)| {
                Command::conditional(start, op, expr)
            }
        ) >> (extended)
    )
);

named!(
    pub commandline<CompleteStr, Command>,
    map!(sp!(separated_list!(separator, list)), |v| Command::group(v))

);

/// Parse a command from a string and panic if there is an error
pub fn must_parse(input: &str) -> Command {
    commandline(CompleteStr(input))
        .unwrap_or_else(|e| panic!("{}", e))
        .1
}

use lang::ast::*;
use lang::word::word;
use lang::word::Word;
use nom;
///! Nom combinations for parsing RUSH shell scripts
use nom::types::CompleteStr;
use std::os::unix::io::RawFd;
use std::str::FromStr;

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
    pub simple_command<CompleteStr, Command>,
    do_parse!(
        args: separated_list!(space, preceded!(not!(io_number), word)) >>
        (Command::simple(args))
    )
);

pub fn split_words<T: AsRef<str>>(s: T) -> Vec<Word> {
    let complete = CompleteStr(s.as_ref());
    separated_list!(complete, space, word)
        .unwrap_or((CompleteStr(""), Vec::new()))
        .1
}

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
    pub group<CompleteStr, Command>,
    sp!(delimited!(char!('{'), sp!(commandline), char!('}')))
);

named!(
    pub redirect<CompleteStr, Command>,
    do_parse!(
        command  : sp!(alt!(function | group | simple_command)) >>
        redirect : opt!(many1!(sp!(redirect_destination))) >>
        (match redirect {
            Some(v) => Command::redirect(command, v),
            None => command,
        })
    )
);

named!(
    pub function<CompleteStr, Command>,
    do_parse!(
        _kw : sp!(tag!("function")) >>
        name : sp!(word) >>
        body : sp!(group) >>
        (Command::Function(Box::new(Function {
            name: name,
            body: body,
        })))
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
            |start, (op, expr)| {
                Command::conditional(start, op, expr)
            }
        ) >> (extended)
    )
);

named!(
    pub comment<CompleteStr, Command>,
    map!(preceded!(tag!("#"), take_until!("\n")), |s| Command::Comment(s.0.to_string()))
);

named!(
    pub commandline<CompleteStr, Command>,
    map!(sp!(separated_list!(separator, list)), |v| Command::group(v))
);

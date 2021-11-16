use crate::lang::word::Word;
use std::os::unix::io::RawFd;
use std::vec::Vec;

#[derive(Debug, Clone)]
pub enum Command {
    SimpleCommand(SimpleCommand),
    Pipeline(Box<Pipeline>),
    FileRedirect(Box<FileRedirect>),
    ConditionalPair(Box<ConditionalPair>),

    Group(Box<CommandGroup>),
    BraceGroup(Box<CommandGroup>),
    SubShell(Box<CommandGroup>),

    If(Box<If>),
    Case(Box<Case>),
    While(Box<While>),
    For(Box<For>),
    Until(Box<Until>),
    Function(Box<Function>),
    Comment(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Separator {
    Stop, // ;
    Fork, // &
    Eol,  // \n
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConditionOperator {
    AndIf, // &&
    OrIf,  // ||
}

#[derive(Debug, Clone)]
pub enum IoOperation {
    Input,             // <
    OutputCreate,      // >
    Output,            // >|
    OutputAppend,      // >>
    HereDocument,      // <<
    HereDocumentStrip, // <<-
    InputDupFd,        // <&
    OutputDupFd,       // &>
    ReadWrite,         // <>
}

#[derive(Debug, Clone)]
pub struct SimpleCommand {
    pub arguments: Vec<Word>,
}

#[derive(Debug, Clone)]
pub struct CommandGroup {
    pub commands: Vec<Command>,
}

#[derive(Debug, Clone)]
pub struct If {
    pub condition: Command,
    pub success: Command,
    pub failure: Command,
}

#[derive(Debug, Clone)]
pub struct While {
    pub condition: Command,
    pub body: Command,
}

#[derive(Debug, Clone)]
pub struct Until {
    pub condition: Command,
    pub body: Command,
}

#[derive(Debug, Clone)]
pub struct For {
    pub condition: Command,
    pub body: Command,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: Word,
    pub body: Command,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub input: Word,
    pub cases: Vec<(Word, Command)>,
}

#[derive(Debug, Clone)]
pub struct Pipeline {
    pub bang: bool,
    pub from: Command,
    pub to: Command,
}

#[derive(Debug, Clone)]
pub struct ConditionalPair {
    pub left: Command,
    pub operator: ConditionOperator,
    pub right: Command,
}

#[derive(Debug, Clone)]
pub struct RedirectDestination {
    pub operation: IoOperation,
    pub fd: Option<RawFd>,
    pub file: Word,
}

#[derive(Debug, Clone)]
pub struct FileRedirect {
    pub left: Command,
    pub redirects: Vec<RedirectDestination>,
}

impl RedirectDestination {
    pub fn new(
        operation: IoOperation,
        fd: Option<RawFd>,
        file: Option<Word>,
    ) -> RedirectDestination {
        RedirectDestination {
            operation: operation,
            fd: fd,
            file: file.unwrap_or(Word::new()),
        }
    }
}

impl<T> From<T> for Command
where
    T: AsRef<str>,
{
    fn from(s: T) -> Command {
        use crate::lang::parser::commandline;
        use nom::types::CompleteStr;
        commandline(CompleteStr(s.as_ref())).unwrap().1
    }
}

impl Command {
    pub fn simple(args: Vec<Word>) -> Command {
        Command::SimpleCommand(SimpleCommand { arguments: args })
    }

    pub fn pipeline(bang: bool, source: Command, dest: Command) -> Command {
        Command::Pipeline(Box::new(Pipeline {
            bang: bang,
            from: source,
            to: dest,
        }))
    }

    pub fn conditional(left: Command, infix: ConditionOperator, right: Command) -> Command {
        Command::ConditionalPair(Box::new(ConditionalPair {
            left: left,
            operator: infix,
            right: right,
        }))
    }

    pub fn redirect(source: Command, redir: Vec<RedirectDestination>) -> Command {
        Command::FileRedirect(Box::new(FileRedirect {
            left: source,
            redirects: redir,
        }))
    }

    pub fn group(source: Vec<Command>) -> Command {
        Command::Group(Box::new(CommandGroup { commands: source }))
    }
}

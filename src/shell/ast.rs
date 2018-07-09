use shell::word::Word;
use std::os::unix::io::RawFd;
use std::vec::Vec;

#[derive(Debug, Clone)]
pub enum Command<'a> {
    SimpleCommand(SimpleCommand<'a>),
    Pipeline(Box<Pipeline<'a>>),
    FileRedirect(Box<FileRedirect<'a>>),
    ConditionalPair(Box<ConditionalPair<'a>>),

    Group(Box<CommandGroup<'a>>),
    BraceGroup(Box<CommandGroup<'a>>),
    SubShell(Box<CommandGroup<'a>>),

    If(Box<If<'a>>),
    Case(Box<Case<'a>>),
    While(Box<While<'a>>),
    For(Box<For<'a>>),
    Until(Box<Until<'a>>),
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
pub struct SimpleCommand<'a> {
    pub arguments: Vec<Word<'a>>,
}

#[derive(Debug, Clone)]
pub struct CommandGroup<'a> {
    pub commands: Vec<Command<'a>>,
}

#[derive(Debug, Clone)]
pub struct If<'a> {
    pub condition: Command<'a>,
    pub success: Command<'a>,
    pub failure: Command<'a>,
}

#[derive(Debug, Clone)]
pub struct While<'a> {
    pub condition: Command<'a>,
    pub body: Command<'a>,
}

#[derive(Debug, Clone)]
pub struct Until<'a> {
    pub condition: Command<'a>,
    pub body: Command<'a>,
}

#[derive(Debug, Clone)]
pub struct For<'a> {
    pub condition: Command<'a>,
    pub body: Command<'a>,
}

#[derive(Debug, Clone)]
pub struct Function<'a> {
    pub name: Word<'a>,
    pub body: Command<'a>,
}

#[derive(Debug, Clone)]
pub struct Case<'a> {
    pub input: Word<'a>,
    pub cases: Vec<(Word<'a>, Command<'a>)>,
}

#[derive(Debug, Clone)]
pub struct Pipeline<'a> {
    pub bang: bool,
    pub from: Command<'a>,
    pub to: Command<'a>,
}

#[derive(Debug, Clone)]
pub struct ConditionalPair<'a> {
    pub left: Command<'a>,
    pub operator: ConditionOperator,
    pub right: Command<'a>,
}

#[derive(Debug, Clone)]
pub struct RedirectDestination<'a> {
    pub operation: IoOperation,
    pub fd: Option<RawFd>,
    pub file: Word<'a>,
}

#[derive(Debug, Clone)]
pub struct FileRedirect<'a> {
    pub left: Command<'a>,
    pub redirects: Vec<RedirectDestination<'a>>,
}

impl<'a> RedirectDestination<'a> {
    pub fn new(
        operation: IoOperation,
        fd: Option<RawFd>,
        file: Option<Word<'a>>,
    ) -> RedirectDestination {
        RedirectDestination {
            operation: operation,
            fd: fd,
            file: file.unwrap_or(Word::new()),
        }
    }
}

impl<'a> Command<'a> {
    pub fn simple(args: Vec<Word<'a>>) -> Command<'a> {
        Command::SimpleCommand(SimpleCommand { arguments: args })
    }

    pub fn pipeline(bang: bool, source: Command<'a>, dest: Command<'a>) -> Command<'a> {
        Command::Pipeline(Box::new(Pipeline {
            bang: bang,
            from: source,
            to: dest,
        }))
    }

    pub fn conditional(
        left: Command<'a>,
        infix: ConditionOperator,
        right: Command<'a>,
    ) -> Command<'a> {
        Command::ConditionalPair(Box::new(ConditionalPair {
            left: left,
            operator: infix,
            right: right,
        }))
    }

    pub fn redirect(source: Command<'a>, redir: Vec<RedirectDestination<'a>>) -> Command<'a> {
        Command::FileRedirect(Box::new(FileRedirect {
            left: source,
            redirects: redir,
        }))
    }

    pub fn group(source: Vec<Command<'a>>) -> Command<'a> {
        Command::Group(Box::new(CommandGroup { commands: source }))
    }
}

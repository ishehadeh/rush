#[macro_use]
extern crate nom;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate failure;
extern crate nix;

pub mod env;
pub mod expr;
pub mod lang;
pub mod term;

use std::ffi::OsString;
use std::io;
use std::io::Write;

pub struct Shell {
    command_buffer: String,
    exit: bool,
}

impl Shell {
    pub fn new() -> Shell {
        Shell {
            command_buffer: String::new(),
            exit: false,
        }
    }

    pub fn readline(&mut self) -> term::Result<String> {
        io::stdout().flush();
        self.command_buffer.clear();

        term::take_terminal(|k| {
            match k {
                term::Key::Control(c) => {
                    if c == 'D' && self.command_buffer.len() == 0 {
                        print!("exit");
                        self.exit = true;
                        return false;
                    }
                    if c == 'C' {
                        print!("^{}", c);
                        self.command_buffer.clear();
                        return false;
                    }
                }
                term::Key::Newline => return false,
                term::Key::Escape => (),
                term::Key::Delete => {
                    if self.command_buffer.len() > 0 {
                        self.command_buffer.pop();
                        term::ansi::cursor_left(1);
                        print!(" ");
                        term::ansi::cursor_left(1);
                    }
                }
                term::Key::Ascii(c) => {
                    self.command_buffer.push(c);
                    print!("{}", c);
                }
                term::Key::Arrow(_) => print!("ESC]"),
                term::Key::Invalid(_) => print!("\u{FFFD}"),
            };
            io::stdout().flush();
            true
        })?;

        Ok(self.command_buffer.clone())
    }

    pub fn exit_requested(&self) -> bool {
        self.exit
    }
}

fn print_error<T: failure::Fail>(e: T) {
    match e.cause() {
        Some(v) => println!("{}: {}", e, v),
        None => println!("{}", e),
    }
}

fn main() {
    let mut shell = Shell::new();
    let mut exe = lang::ExecutionEnvironment::new();
    let prefix_command = exe
        .variables()
        .value(&OsString::from("RUSH_PREFIX"))
        .to_string_lossy()
        .to_string();

    let prefix = lang::ast::Command::from(if prefix_command.is_empty() {
        "printf 'rush-%s$ ' \"$RUSH_VERSION\"".to_string()
    } else {
        prefix_command
    });

    exe.variables_mut().define("RUSH_VERSION", "0.1.0");

    while !shell.exit_requested() {
        exe.run(prefix.clone());
        let buffer = match shell.readline() {
            Ok(v) => v,
            Err(e) => {
                println!();
                print_error(e);
                continue;
            }
        };
        if !shell.exit_requested() {
            println!();

            if !buffer.is_empty() {
                match exe.run(buffer) {
                    Err(e) => {
                        print_error(e);
                        continue;
                    }
                    _ => (),
                }
            }
        }
    }
}

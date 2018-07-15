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
pub mod shell;
pub mod term;

use std::env::args;
use std::fs;
use std::io;
use std::io::Read;
use std::io::Write;
use std::process::exit;
use term::terminfo;

fn main() {
    if !terminfo::found_terminfo() {
        term::ansi::effect(term::ansi::Effect::Foreground(3));
        term::ansi::effect(term::ansi::Effect::Bold);
        print!("warning: ");
        term::ansi::effect(term::ansi::Effect::Foreground(15));
        print!("failed to find terminal information, using built-in defaults.");
        term::ansi::effect(term::ansi::Effect::DefaultForeground);
        term::ansi::effect(term::ansi::Effect::UnsetBoldAndFaint);
        println!();
    }

    let mut shell = shell::Shell::new();
    let mut environ = lang::ExecutionEnvironment::new();

    environ.variables_mut().define("RUSH_VERSION", "0.1.0");

    match args().nth(1) {
        Some(v) => exit(environ.run(v).unwrap_or_else(|e| {
            println!("{}", e);
            1
        })),
        None => shell.run(&mut environ),
    }
}

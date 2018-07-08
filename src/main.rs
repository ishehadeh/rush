#[macro_use]
extern crate nom;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate lazy_static;
extern crate nix;

pub mod env;
pub mod expr;
pub mod shell;

use std::env::args;

fn main() {
    let arg1 = args().next_back().unwrap();
    let mut exe = shell::ExecutionEnvironment::new();
    println!(
        "\"{}\" exited with exit code {}",
        arg1,
        exe.execute_str(&arg1)
    );
}

#[macro_use]
extern crate nom;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate failure;
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
        exe.run_str(&arg1).unwrap_or_else(|e| panic!("{}", e))
    );
}

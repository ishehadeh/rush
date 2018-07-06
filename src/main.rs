#[macro_use]
extern crate nom;
#[macro_use]
extern crate error_chain;
extern crate nix;

pub mod expr;
pub mod parser;
pub mod scope;
pub mod variables;

use std::env::args;

fn main() {
    let arg1 = args().nth(1).unwrap();
    let mut env = scope::ExecutionEnvironment::new();
    env.inherit_environment().unwrap();
    println!("{}", env.expand_word(arg1).iter().nth(0).unwrap());
}

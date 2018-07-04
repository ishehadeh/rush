#[macro_use]
extern crate nom;
extern crate nix;

pub mod parser;
pub mod scope;
pub mod variables;

use std::env::args;

fn main() {
    let arg1 = args().nth(1).unwrap();
    let mut scope = scope::ExecutionEnvironment::new();
    scope.inherit_environment().unwrap();
    scope.variables_mut().define("TEST", "HI");
    println!("{:?}", scope.expand_word(arg1));
}

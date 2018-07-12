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

fn main() {
    let mut shell = shell::Shell::new();
    let mut environ = lang::ExecutionEnvironment::new();

    environ.variables_mut().define("RUSH_VERSION", "0.1.0");

    shell.run(&mut environ);
}

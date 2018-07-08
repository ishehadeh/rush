pub mod ast;
mod errors;
pub mod parser;
pub mod word;

pub use self::errors::*;
use env;
use nix;
use nom;
use std::env::split_paths;
use std::ffi::{CString, OsString};
use std::path;
use std::vec::Vec;

pub struct ExecutionEnvironment<'a> {
    funcs: env::Functions<'a>,
    vars: env::Variables,
}

impl<'a> ExecutionEnvironment<'a> {
    pub fn new() -> ExecutionEnvironment<'a> {
        ExecutionEnvironment {
            funcs: env::Functions::new(),
            vars: env::Variables::from_env(),
        }
    }

    pub fn find_executable(&self, prog: &String) -> Option<path::PathBuf> {
        for path in split_paths(&self.vars.value(&OsString::from("PATH"))) {
            let p = path.with_file_name(prog);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    pub fn execute(&mut self, cmd: ast::SimpleCommand<'a>) -> i32 {
        match nix::unistd::fork().unwrap() {
            nix::unistd::ForkResult::Child => {
                let args = cmd
                    .arguments
                    .iter()
                    .map(|arg| CString::new(arg.compile(&mut self.vars)).unwrap())
                    .collect::<Vec<CString>>();
                let arg0 = &cmd
                    .arguments
                    .iter()
                    .nth(0)
                    .clone()
                    .unwrap()
                    .compile(&mut self.vars);
                let exe = self.find_executable(arg0).unwrap();
                nix::unistd::execv(&CString::new(exe.to_str().unwrap()).unwrap(), &args).unwrap();
                unreachable!()
            }
            nix::unistd::ForkResult::Parent { child } => {
                match nix::sys::wait::waitpid(child, None).unwrap() {
                    nix::sys::wait::WaitStatus::Exited(_, status) => status,
                    _ => unimplemented!(),
                }
            }
        }
    }

    pub fn execute_str(&mut self, s: &'a str) -> i32 {
        let cmd = parser::simple_command(nom::types::CompleteStr(s))
            .unwrap()
            .1;
        self.execute(match cmd {
            ast::Command::SimpleCommand(s) => s,
            _ => unreachable!(),
        })
    }
}

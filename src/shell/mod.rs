pub mod ast;
mod errors;
pub mod parser;
pub mod word;

pub use self::errors::*;
use env;
use failure::ResultExt;

use nix;
use nom;
use std::env::split_paths;
use std::ffi::{CString, OsStr, OsString};
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

    pub fn find_executable<S: AsRef<OsStr>>(&self, prog: S) -> Result<path::PathBuf> {
        let prog_ref = prog.as_ref();
        for path in split_paths(&self.vars.value(&OsString::from("PATH"))) {
            let p = path.with_file_name(prog_ref);
            if p.exists() {
                return Ok(p);
            }
        }

        let owned_prog = prog_ref.to_os_string().to_string_lossy().to_string();
        Err(Error::from(ErrorKind::MissingExecutable(owned_prog)))
    }

    pub fn execute(&mut self, cmd: ast::SimpleCommand<'a>) -> Result<i32> {
        match nix::unistd::fork().unwrap() {
            nix::unistd::ForkResult::Child => {
                let compiled_args = cmd.arguments
                    .iter()
                    .map(|arg| arg.compile(&mut self.vars))
                    .collect::<Vec<String>>();

                let exe = self.find_executable(compiled_args.first().unwrap())?;
                let mut cargs = Vec::with_capacity(compiled_args.len());
                for x in compiled_args {
                    cargs.push(CString::new(x).context(ErrorKind::IllegalNullByte)?)
                }
                nix::unistd::execv(
                    &CString::new(exe.to_string_lossy().to_string())
                        .context(ErrorKind::IllegalNullByte)?,
                    &cargs,
                ).context(ErrorKind::ExecFailed)?;
                unreachable!()
            }
            nix::unistd::ForkResult::Parent { child } => {
                match nix::sys::wait::waitpid(child, None).context(ErrorKind::WaitFailed)? {
                    nix::sys::wait::WaitStatus::Exited(_, status) => Ok(status),
                    _ => unimplemented!(),
                }
            }
        }
    }

    pub fn execute_str(&mut self, s: &'a str) -> Result<i32> {
        let cmd = parser::simple_command(nom::types::CompleteStr(s))
            .unwrap()
            .1;
        self.execute(match cmd {
            ast::Command::SimpleCommand(s) => s,
            _ => unreachable!(),
        })
    }
}

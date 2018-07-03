use nix::unistd;
use nom;
use nom::types::CompleteStr;
use parser;
use parser::*;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::env;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::process;
use std::string::String;
use std::sync::{Arc, Mutex};

type Signal = i32;

pub struct ExecutionEnvironment {
    pwd: PathBuf,
    directory_stack: VecDeque<PathBuf>,
    variables: HashMap<String, String>,
    functions: HashMap<String, Command>,
    traps: HashMap<Signal, Command>,
    aliases: HashMap<String, String>,
    files: Vec<File>,
}

impl ExecutionEnvironment {
    pub fn new() -> ExecutionEnvironment {
        ExecutionEnvironment {
            pwd: PathBuf::new(),
            files: Vec::new(),
            traps: HashMap::new(),
            directory_stack: VecDeque::new(),
            variables: HashMap::new(),
            functions: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn inherit_environment(&mut self) -> io::Result<()> {
        for (k, v) in env::vars() {
            self.variables.insert(k, v);
        }

        self.change_directory(env::current_dir()?);
        Ok(())
    }

    /// The the current working directory ($PWD)
    pub fn change_directory<T: Into<PathBuf>>(&mut self, pb: T) {
        self.pwd = pb.into();
        self.variables
            .insert("PWD".to_string(), self.pwd.to_string_lossy().to_string());
    }

    /// Get the current working directory (same as $PWD)
    pub fn current_directory(&self) -> PathBuf {
        self.pwd.clone()
    }

    /// push a directory onto the stack
    pub fn push_directory<T: Into<PathBuf>>(&mut self, pb: T) {
        self.directory_stack.push_back(pb.into());
    }

    /// try to pop a directory from the stack, if it exists set it as the working directory
    pub fn pop_directory<T: Into<PathBuf>>(&mut self) {
        match self.directory_stack.pop_back() {
            Some(v) => self.change_directory(v),
            None => (),
        }
    }

    pub fn child(&self) -> ExecutionEnvironment {
        ExecutionEnvironment {
            pwd: self.pwd.clone(),
            files: Vec::new(),
            traps: HashMap::new(),
            directory_stack: VecDeque::new(),
            variables: self.variables.clone(),
            functions: self.functions.clone(),
            aliases: self.aliases.clone(),
        }
    }

    pub fn home(&self) -> String {
        self.variables.get("HOME").map(|v| v.clone()).unwrap_or(
            env::home_dir()
                .map(|v| v.to_string_lossy().to_string())
                .unwrap_or(String::new()),
        )
    }

    pub fn variable<T: Into<String>>(&self, var: T) -> String {
        self.variables
            .get(&var.into())
            .map(|v| v.clone())
            .unwrap_or(String::new())
    }

    pub fn variable_length<T: Into<String>>(&self, var: T) -> usize {
        match self.variables.get(&var.into()) {
            Some(v) => v.len(),
            None => 0,
        }
    }

    /// Expand a word into a series of fields
    ///
    /// TODO: detailed explanation
    pub fn expand_word(&mut self, w: Word) -> Vec<String> {
        let tilde = self.expand_tilde(CompleteStr(&w)).unwrap().1;
        let vars = self.expand_variables(CompleteStr(&tilde)).unwrap().1;
        vec![vars]
    }

    fn expand_tilde<'a>(&self, i: CompleteStr<'a>) -> nom::IResult<CompleteStr<'a>, String, u32> {
        ws!(
            i,
            map!(
                many0!(alt!(
                  char!('~')   => { |_| self.home() }
                | recognize!(parser::single_quoted_string) => { |v : CompleteStr| v.0.to_string() }
                | recognize!(parser::double_quoted_string) => { |v : CompleteStr| v.0.to_string() }
                | recognize!(parser::unquoted_string)      => { |v : CompleteStr| v.0.to_string() }
            )),
                |v| v.join("")
            )
        )
    }

    fn expand_variables<'a>(
        &self,
        i: CompleteStr<'a>,
    ) -> nom::IResult<CompleteStr<'a>, String, u32> {
        ws!(
            i,
            map!(
                many0!(alt!(
                      preceded!(char!('$'), 
                        alt!(
                            variable_name => { |k : CompleteStr| self.variable(k.0) }
                            | delimited!(
                                char!('{'),
                                    alt!(
                                          preceded!(char!('#'), variable_name) => { |k : CompleteStr| self.variable_length(k.0).to_string() }
                                        | variable_name => { |k : CompleteStr| self.variable(k.0) }
                                    ),
                                char!('}')
                            )
                        )) => { |v| v }
                    | recognize!(parser::single_quoted_string) => { |v : CompleteStr| v.0.to_string() }
                    | take_while!(|c| c != '$') => { |v : CompleteStr| v.0.to_string() }
                )),
                |v| v.join("")
            )
        )
    }
}

named!(
    variable_name<CompleteStr, CompleteStr>,
    take_while1!(|c| nom::is_alphanumeric(c as u8) || c == '_')
);

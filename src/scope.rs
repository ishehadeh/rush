use expr;
use nix::unistd;
use nom;
use nom::types::CompleteStr;
use parser;
use parser::*;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use std::process;
use std::str::FromStr;
use std::string::String;
use variables::Variables;

macro_rules! parameter_operation {
    ($i:ident, $op:expr) => {
        tuple!($i, variable_name, preceded!(tag!($op), param_word))
    };
}

macro_rules! env_call {
    ($i:ident, $_self:ident. $fun:ident) => {
        $_self.$fun($i)
    };
}

type Signal = i32;

pub struct ExecutionEnvironment {
    pwd: PathBuf,
    directory_stack: VecDeque<PathBuf>,
    variables: Variables,
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
            variables: Variables::new(),
            functions: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    pub fn inherit_environment(&mut self) -> io::Result<()> {
        self.variables.import_env();
        self.change_directory(env::current_dir()?);
        Ok(())
    }

    pub fn variables_mut<'a>(&'a mut self) -> &'a mut Variables {
        &mut self.variables
    }

    pub fn variables<'a>(&'a self) -> &'a Variables {
        &self.variables
    }

    /// change the current working directory ($PWD)
    pub fn change_directory<T: Into<PathBuf>>(&mut self, pb: T) {
        let v = pb.into();
        self.pwd = v.clone();
        self.variables_mut().define("PWD", v);
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
        let home_def = self.variables().value("HOME");
        if home_def.len() > 0 {
            home_def
        } else {
            env::home_dir()
                .map(|v| v.into_os_string())
                .unwrap_or(OsString::new())
        }.into_string()
            .unwrap()
    }

    /// Expand a word into a series of fields
    ///
    /// TODO: detailed explanation
    pub fn expand_word(&mut self, w: Word) -> Vec<String> {
        vec![self.basic_word_expansion(CompleteStr(&w)).unwrap().1]
    }

    fn get_numeric_variable(&self, name: String) -> f64 {
        f64::from_str(&self.variables().value(name).into_string().unwrap()).unwrap()
    }

    fn expand_parameter<'a>(
        &mut self,
        i: CompleteStr<'a>,
    ) -> nom::IResult<CompleteStr<'a>, String, u32> {
        delimited!(
            i,
            char!('{'),
            alt!(
                  preceded!(char!('#'), variable_name) => { |k : CompleteStr| self.variables().value(k.0).len().to_string() }
                | parameter_operation!("=")  => { |(k, v) : (CompleteStr, CompleteStr)| self.variables_mut().entry(k.0).or_insert(v.0).clone().into_string().unwrap() }
                | parameter_operation!(":=") => { |(k, v) : (CompleteStr, CompleteStr)| self.variables_mut().entry(k.0).or_insert_null(v.0).clone().into_string().unwrap() }
                | parameter_operation!("-")  => { |(k, v) : (CompleteStr, CompleteStr)| self.variables_mut().entry(k.0).default(v.0).clone().into_string().unwrap() }
                | parameter_operation!(":-") => { |(k, v) : (CompleteStr, CompleteStr)| self.variables_mut().entry(k.0).default_null(v.0).clone().into_string().unwrap() }
                | parameter_operation!("?")  => { |(k, v) : (CompleteStr, CompleteStr)| 
                    {
                        if !self.variables().exists(k.0) {
                            panic!("${} is not set!", k.0);
                        }
                        self.variables().value(k.0).clone().into_string().unwrap()
                    }
                }
                | parameter_operation!(":?")  => { |(k, v) : (CompleteStr, CompleteStr)| 
                    {
                        if !self.variables().has_value(k.0) {
                            panic!("${} is not set!", k.0);
                        }
                        self.variables().value(k.0).clone().into_string().unwrap()
                    }
                }
                | parameter_operation!(":+")  => { |(k, v) : (CompleteStr, CompleteStr)| 
                    {
                        if !self.variables().has_value(k.0) {
                            String::new()
                        } else {
                            v.to_string()
                        }
                    }
                }
                | parameter_operation!("+")  => { |(k, v) : (CompleteStr, CompleteStr)|
                    {
                        if !self.variables().exists(k.0) {
                            String::new()
                        } else {
                            v.to_string()
                        }
                    }
                }
                | variable_name => { |k : CompleteStr| self.variables().value(k.0).clone().into_string().unwrap() }
            ),
            char!('}')
        )
    }

    fn basic_word_expansion<'a>(
        &mut self,
        i: CompleteStr<'a>,
    ) -> nom::IResult<CompleteStr<'a>, String, u32> {
        ws!(
            i,
            do_parse!(
                maybe_tilde:
                    opt!(preceded!(
                        char!('~'),
                        peek!(terminated!(valid_path_string, one_of!("\\/")))
                    ))
                    >> rest: map!(
                        many0!(alt!(
                            preceded!(char!('$'), 
                                alt!(
                                    variable_name => { |k : CompleteStr| self.variables().value(k.0).clone().into_string().unwrap() }
                                    | delimited!(tag!("(("), escaped!(alt!(take_until_either1!("()") | delimited!(char!('('), is_not!(")"), char!(')'))), '\\', one_of!("\\()")), tag!("))")) => { |e : CompleteStr| expr::eval(&self.expand_word(e.0.to_string()).join(""), self.variables_mut()).unwrap() }
                                    | env_call!(self.expand_parameter) => { |k| k }
                                )) => { |v| v }
                            | recognize!(parser::single_quoted_string) => { |v : CompleteStr| v.0.to_string() }
                            | take_while!(|c| c != '$') => { |v : CompleteStr| v.0.to_string() }
                        )),
                        |v| v.join("")
                    ) >> (match maybe_tilde {
                    Some(_) => {
                        let mut home = self.home();
                        home.push_str(&rest);
                        home
                    }
                    None => rest,
                })
            )
        )
    }
}

named!(
    variable_name<CompleteStr, CompleteStr>,
    take_while1!(|c| nom::is_alphanumeric(c as u8) || c == '_')
);

named!(
    unquoted_param_string<CompleteStr, CompleteStr>,
    preceded!(not!(io_number), escaped!(is_not!(" }\\'\"()|&;<>\t\n"), '\\', one_of!(" }\\'\"()|&;<>\t\n~")))
);

named!(
    valid_path_string<CompleteStr, CompleteStr>,
    preceded!(not!(io_number), escaped!(is_not!(" {/\\'\"()|&;<>\t\n"), '\\', one_of!(" {/\\'\"()|&;<>\t\n~")))
);

named!(
    param_word<CompleteStr, CompleteStr>,
    recognize!(
        alt!(
              single_quoted_string
            | double_quoted_string
            | unquoted_param_string
        )
    )
);

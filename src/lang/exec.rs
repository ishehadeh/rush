use crate::env::functions::Functions;
use crate::env::traps;
use crate::env::variables::Variables;
use crate::jobs::spawn::ProcessOptions;
use crate::lang::ast::Command;
use crate::lang::ast::ConditionOperator;
use crate::lang::word::Word;
use crate::lang::{Error, ErrorKind, Result};
use failure::ResultExt;
use nix::libc;
use nix::sys::signal;
use nix::sys::wait::{wait, WaitStatus};
use nix::unistd;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::{CString, OsStr, OsString};
use std::os::unix::io::RawFd;
use std::path::PathBuf;

#[derive(Debug, Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Jid(u32);

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub cwd: PathBuf,
    vars: Variables,
    funcs: Functions,
}

#[derive(Copy, Clone, Debug)]
pub struct ExitStatus {
    pub pid: unistd::Pid,
    pub exit_code: i32,
    pub core_dumped: bool,
    pub signal: Option<signal::Signal>,
}

pub enum JobStatus {
    Running,
    Complete(ExitStatus),
}

pub struct JobManager {
    next_jid: u32,
    running_jobs: BTreeMap<libc::pid_t, Jid>,
    completed_jobs: BTreeMap<Jid, ExitStatus>,
}

struct ProcOptions<'a> {
    close_fds: &'a Vec<RawFd>,
    env: &'a [(String, String)],
    stdin: Option<RawFd>,
    stdout: Option<RawFd>,
}

impl Default for JobManager {
    fn default() -> Self {
        JobManager {
            next_jid: 0,
            running_jobs: BTreeMap::new(),
            completed_jobs: BTreeMap::new(),
        }
    }
}

impl JobManager {
    pub fn new() -> JobManager {
        Self::default()
    }

    pub fn run(&mut self, ec: &mut ExecutionContext, command: Command) -> Result<ExitStatus> {
        let close_fds = Vec::new();
        let env = Vec::new();
        let opts = ProcOptions {
            stdin: None,
            stdout: None,
            close_fds: &close_fds,
            env: &env,
        };

        let jids = self.spawn_procs_from_ast(&opts, ec, &command)?;
        self.await_all(&jids)?;
        Ok(jids
            .last()
            .map(|id| *self.completed_jobs.get(id).unwrap())
            .unwrap_or(ExitStatus {
                exit_code: 0,
                core_dumped: false,
                pid: unistd::getpid(),
                signal: None,
            }))
    }

    fn next(&mut self) -> Result<(Jid, ExitStatus)> {
        let mut status = None;
        while status.is_none() {
            match wait().context(ErrorKind::WaitFailed)? {
                WaitStatus::Exited(pid, code) => {
                    status = self.running_jobs.get(&pid.into()).map(|jid| {
                        (
                            *jid,
                            ExitStatus {
                                pid,
                                exit_code: code,
                                core_dumped: false,
                                signal: None,
                            },
                        )
                    });
                }
                WaitStatus::Signaled(pid, sig, core_dump) => {
                    status = self.running_jobs.get(&pid.into()).map(|jid| {
                        (
                            *jid,
                            ExitStatus {
                                pid,
                                exit_code: -1,
                                core_dumped: core_dump,
                                signal: Some(sig),
                            },
                        )
                    });
                }
                _ => (),
            }
        }

        Ok(status.unwrap())
    }

    fn add_job(&mut self, pid: unistd::Pid) -> Jid {
        let jid = Jid(self.next_jid);
        self.running_jobs.insert(pid.into(), jid);
        self.next_jid += 1;
        jid
    }

    // spawn 0 or more processes based on a shell-language abstract syntax tree in a given execution context
    fn spawn_procs_from_ast<'a>(
        &mut self,
        opts: &'a ProcOptions<'a>,
        ec: &mut ExecutionContext,
        command: &Command,
    ) -> Result<Vec<Jid>> {
        match command {
            Command::SimpleCommand(cmd) => {
                // TODO: make sure theres at least 1 argument
                let argv0 = cmd.arguments[0]
                    .compile(ec.variables_mut())
                    .context(ErrorKind::ExecFailed)?;

                if let Some(body) = ec.functions().value(&argv0) {
                    self.spawn_procs_from_ast(opts, ec, &body)
                } else {
                    let mut proc = if argv0.starts_with("./") {
                        ProcessOptions::new(&argv0)
                    } else {
                        let executable = ec.find_executable(&argv0)?.to_string_lossy().to_string();
                        ProcessOptions::new(&executable)
                    };

                    // The first argument is the command used to run the executable
                    // Avoid compiling it again since that can have side effects (e.g. "./exe$(exe += 1))")
                    proc.arg(&argv0);
                    for arg in cmd.arguments.iter().skip(1) {
                        proc.arg(
                            &arg.compile(ec.variables_mut())
                                .context(ErrorKind::ExecFailed)?,
                        );
                    }

                    for (k, v) in opts.env {
                        proc.env(k, v);
                    }

                    proc.work_dir(&ec.cwd);

                    if let Some(stdin) = opts.stdin {
                        proc.redirect(stdin, 0);
                        proc.close(stdin);
                    }

                    if let Some(stdout) = opts.stdout {
                        proc.redirect(stdout, 1);
                        proc.close(stdout);
                    }
                    for &close in opts.close_fds {
                        proc.close(close);
                    }
                    let pid = proc.spawn().context(ErrorKind::ExecFailed)?;

                    Ok(vec![self.add_job(pid)])
                }
            }
            Command::Pipeline(pipe) => {
                let (stdin, stdout) = unistd::pipe().context(ErrorKind::PipelineCreationFailed)?;
                let mut close_from = opts.close_fds.clone();
                let mut to_from = opts.close_fds.clone();

                close_from.push(stdin);
                if let Some(pipe_out) = opts.stdout {
                    close_from.push(pipe_out)
                }
                to_from.push(stdout);
                if let Some(pipe_in) = opts.stdin {
                    to_from.push(pipe_in)
                }

                let from_opts = ProcOptions {
                    close_fds: &close_from,
                    env: opts.env,
                    stdin: opts.stdin,
                    stdout: Some(stdout),
                };

                let to_opts = ProcOptions {
                    close_fds: &to_from,
                    env: opts.env,
                    stdin: Some(stdin),
                    stdout: opts.stdout,
                };

                let mut jids = self.spawn_procs_from_ast(&from_opts, ec, &pipe.from)?;
                jids.extend(self.spawn_procs_from_ast(&to_opts, ec, &pipe.to)?);

                unistd::close(stdin).context(ErrorKind::ExecFailed)?;
                unistd::close(stdout).context(ErrorKind::ExecFailed)?;

                Ok(jids)
            }
            Command::BraceGroup(group) => {
                let mut subenv = ec.clone();
                for cmd in &group.commands {
                    let jids = self.spawn_procs_from_ast(opts, &mut subenv, cmd)?;
                    self.await_all(&jids)?;
                }
                Ok(Vec::new())
            }
            Command::Group(group) => {
                for cmd in &group.commands {
                    let jids = self.spawn_procs_from_ast(opts, ec, cmd)?;
                    self.await_all(&jids)?;
                }
                Ok(Vec::new())
            }
            Command::ConditionalPair(cond) => {
                let jobs_left = self.spawn_procs_from_ast(opts, ec, &cond.left)?;
                self.await_all(&jobs_left)?;
                let exit_code = jobs_left
                    .last()
                    .map(|r| self.completed_jobs.get(r).unwrap().exit_code)
                    .unwrap_or(0);
                if (exit_code == 0 && cond.operator == ConditionOperator::AndIf)
                    || (exit_code != 0 && cond.operator == ConditionOperator::OrIf)
                {
                    let jobs_right = self.spawn_procs_from_ast(opts, ec, &cond.right)?;
                    self.await_all(&jobs_right)?;
                    Ok(jobs_right)
                } else {
                    Ok(jobs_left)
                }
            }
            Command::Function(func) => {
                let str_name = func.name.compile(ec.variables_mut())?;
                ec.functions_mut().insert(str_name, func.body.clone());
                Ok(vec![])
            }
            Command::Comment(_s) => Ok(vec![]),
            _ => unimplemented!(),
        }
    }

    pub fn stat(&mut self, jid: Jid) -> Result<JobStatus> {
        if let Some(status) = self.completed_jobs.get(&jid) {
            Ok(JobStatus::Complete(*status))
        } else {
            self.running_jobs
                .iter()
                .find(|(_, v)| **v == jid)
                .map_or(Err(ErrorKind::InvalidJobId(jid).into()), |v| {
                    Ok(JobStatus::Running)
                })
        }
    }

    /// Wait for a specific job to complete
    pub fn r#await(&mut self, jid: Jid) -> Result<ExitStatus> {
        if let Some(exit_status) = self.completed_jobs.get(&jid) {
            return Ok(*exit_status);
        }

        let mut completed = self.next()?;
        while completed.0 != jid {
            self.completed_jobs.insert(completed.0, completed.1);
            completed = self.next()?;
        }
        self.completed_jobs.insert(completed.0, completed.1);
        Ok(completed.1)
    }

    /// Wait for several jobs to complete
    pub fn await_all(&mut self, jids: &[Jid]) -> Result<()> {
        let mut incomplete: BTreeSet<Jid> = jids
            .iter()
            .copied()
            .filter(|jid| self.completed_jobs.get(jid).is_none())
            .collect();

        while !incomplete.is_empty() {
            let completed = self.next()?;

            incomplete.remove(&completed.0);
            self.completed_jobs.insert(completed.0, completed.1);
        }

        Ok(())
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        ExecutionContext {
            vars: Variables::from_env(),
            funcs: Functions::new(),
            cwd: env::current_dir().unwrap(),
        }
    }
}

impl ExecutionContext {
    pub fn new() -> ExecutionContext {
        Self::default()
    }

    pub fn variables(&self) -> &Variables {
        &self.vars
    }

    pub fn variables_mut(&mut self) -> &mut Variables {
        &mut self.vars
    }

    pub fn functions(&self) -> &Functions {
        &self.funcs
    }

    pub fn functions_mut(&mut self) -> &mut Functions {
        &mut self.funcs
    }

    pub fn find_executable<S: AsRef<OsStr>>(&self, prog: S) -> Result<PathBuf> {
        let prog_ref = prog.as_ref();
        for path in env::split_paths(&self.vars.value(&OsString::from("PATH"))) {
            let p = path.join(prog_ref);
            if p.exists() {
                return Ok(p);
            }
        }

        let owned_prog = prog_ref.to_os_string().to_string_lossy().to_string();
        Err(Error::from(ErrorKind::MissingExecutable(owned_prog)))
    }
}

#[cfg(test)]
mod test {
    use std::{
        fs::File,
        io::{self, Read},
    };

    use crate::{
        lang::{
            ast::{Command, CommandGroup, ConditionOperator, Function},
            word::Word,
        },
        test_util::forks,
    };

    use super::{ExecutionContext, JobManager};

    #[test]
    fn simple_command() {
        forks!();

        let mut ec = ExecutionContext::new();
        let mut jm = JobManager::new();
        let status = jm
            .run(&mut ec, Command::simple(vec![Word::parse("true")]))
            .expect("failed to execute 'true'");

        assert_eq!(status.exit_code, 0);
    }

    #[test]
    fn pipeline() {
        forks!();

        let out_file = "test/data/pipeline-out.txt";
        match std::fs::remove_file(&out_file) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(err) => panic!("failed to remove file: {}", err),
        }

        let mut ec = ExecutionContext::new();
        let mut jm = JobManager::new();
        let status = jm
            .run(
                &mut ec,
                Command::pipeline(
                    false,
                    Command::simple(vec![
                        Word::parse("printf"),
                        Word::parse("%s"),
                        Word::parse("hello"),
                    ]),
                    Command::simple(vec![
                        Word::parse("cp"),
                        Word::parse("/dev/stdin"),
                        Word::parse(out_file),
                    ]),
                ),
            )
            .expect("failed to execute pipeline");

        assert_eq!(status.exit_code, 0);

        let mut content = String::new();
        File::open(out_file)
            .expect("failed to open out file")
            .read_to_string(&mut content)
            .expect("failed to read out file");
        assert_eq!(content, "hello");
    }

    #[test]
    fn cond_and() {
        forks!();

        let mut ec = ExecutionContext::new();
        let mut jm = JobManager::new();
        let status = jm
            .run(
                &mut ec,
                Command::conditional(
                    Command::simple(vec![Word::parse("true")]),
                    ConditionOperator::AndIf,
                    Command::simple(vec![Word::parse("true")]),
                ),
            )
            .expect("failed to execute true && true");
        assert_eq!(status.exit_code, 0);

        let status = jm
            .run(
                &mut ec,
                Command::conditional(
                    Command::simple(vec![Word::parse("true")]),
                    ConditionOperator::AndIf,
                    Command::simple(vec![Word::parse("false")]),
                ),
            )
            .expect("failed to execute true && false");
        assert_eq!(status.exit_code, 1);
    }

    #[test]
    fn cond_or() {
        forks!();

        let mut ec = ExecutionContext::new();
        let mut jm = JobManager::new();
        let status = jm
            .run(
                &mut ec,
                Command::conditional(
                    Command::simple(vec![Word::parse("true")]),
                    ConditionOperator::OrIf,
                    Command::simple(vec![Word::parse("true")]),
                ),
            )
            .expect("failed to execute true || true");
        assert_eq!(status.exit_code, 0);

        let status = jm
            .run(
                &mut ec,
                Command::conditional(
                    Command::simple(vec![Word::parse("true")]),
                    ConditionOperator::OrIf,
                    Command::simple(vec![Word::parse("false")]),
                ),
            )
            .expect("failed to execute true || false");
        assert_eq!(status.exit_code, 0);

        let status = jm
            .run(
                &mut ec,
                Command::conditional(
                    Command::simple(vec![Word::parse("false")]),
                    ConditionOperator::OrIf,
                    Command::simple(vec![Word::parse("false")]),
                ),
            )
            .expect("failed to execute false || false");
        assert_eq!(status.exit_code, 1);
    }

    #[test]
    fn group_pipeline() {
        forks!();

        let out_file = "test/data/group_pipeline-out.txt";
        match std::fs::remove_file(&out_file) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(err) => panic!("failed to remove file: {}", err),
        }

        let mut ec = ExecutionContext::new();
        let mut jm = JobManager::new();
        let status = jm
            .run(
                &mut ec,
                Command::pipeline(
                    false,
                    Command::group(vec![
                        Command::simple(vec![Word::parse("printf"), Word::parse("hello\\n")]),
                        Command::simple(vec![Word::parse("printf"), Word::parse("world")]),
                    ]),
                    Command::simple(vec![
                        Word::parse("cp"),
                        Word::parse("/dev/stdin"),
                        Word::parse(out_file),
                    ]),
                ),
            )
            .expect("failed to execute true || true");
        assert_eq!(status.exit_code, 0);

        let mut content = String::new();
        File::open(out_file)
            .expect("failed to open out file")
            .read_to_string(&mut content)
            .expect("failed to read out file");
        assert_eq!(content, "hello\nworld");
    }

    #[test]
    fn expands_vars() {
        forks!();

        std::env::set_var("EXPAND_ENV_VARS_TEST", "helloworld");
        let mut ec = ExecutionContext::new();
        let mut jm = JobManager::new();

        let status = jm
            .run(
                &mut ec,
                Command::simple(vec![
                    Word::parse("test"),
                    Word::parse("$EXPAND_ENV_VARS_TEST"),
                    Word::parse("="),
                    Word::parse("helloworld"),
                ]),
            )
            .expect("failed to execute printf");
        assert_eq!(status.exit_code, 0);

        ec.variables_mut()
            .define("EXPAND_ENV_VARS_TEST", "shadowed");
        let status = jm
            .run(
                &mut ec,
                Command::simple(vec![
                    Word::parse("test"),
                    Word::parse("$EXPAND_ENV_VARS_TEST"),
                    Word::parse("="),
                    Word::parse("shadowed"),
                ]),
            )
            .expect("failed to execute printf");
        assert_eq!(status.exit_code, 0);
    }

    #[test]
    fn function_call_pipeline() {
        forks!();

        let mut ec = ExecutionContext::new();
        let mut jm = JobManager::new();

        let out_file = "test/data/function_call_pipeline-out.txt";
        match std::fs::remove_file(&out_file) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(err) => panic!("failed to remove file: {}", err),
        }

        let status = jm
            .run(
                &mut ec,
                Command::Function(Box::new(Function {
                    name: Word::parse("write_hello_3"),
                    body: Command::BraceGroup(Box::new(CommandGroup {
                        commands: vec![
                            Command::simple(vec![Word::parse("printf"), Word::parse("hello\\n")]),
                            Command::simple(vec![Word::parse("printf"), Word::parse("hello\\n")]),
                            Command::simple(vec![Word::parse("printf"), Word::parse("hello\\n")]),
                        ],
                    })),
                })),
            )
            .expect("failed to execute function statement");
        assert_eq!(status.exit_code, 0);

        ec.variables_mut()
            .define("EXPAND_ENV_VARS_TEST", "shadowed");
        let status = jm
            .run(
                &mut ec,
                Command::pipeline(
                    false,
                    Command::simple(vec![Word::parse("write_hello_3")]),
                    Command::simple(vec![
                        Word::parse("cp"),
                        Word::parse("/dev/stdin"),
                        Word::parse(out_file),
                    ]),
                ),
            )
            .expect("failed to run function");
        assert_eq!(status.exit_code, 0);

        let mut content = String::new();
        File::open(out_file)
            .expect("failed to open out file")
            .read_to_string(&mut content)
            .expect("failed to read out file");
        assert_eq!(content, "hello\nhello\nhello\n");
    }
}

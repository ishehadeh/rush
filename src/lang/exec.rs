use env;
use env::traps;
use failure::ResultExt;
use lang;
use lang::ast;
use lang::word;
use lang::{Error, ErrorKind, Result};
use nix::sys::signal;
use nix::sys::wait;
use nix::unistd;
use std::collections::{HashMap, VecDeque};
use std::env::split_paths;
use std::ffi::CString;
use std::ffi::{OsStr, OsString};
use std::os::unix::io::RawFd;
use std::path;
use std::process;
use std::vec::Vec;

pub type JobId = usize;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum JobStatus {
    Running,
    Finished(i32),
    Stopped(signal::Signal),
    Sleeping,
}

#[derive(Debug, Clone)]
pub enum Action {
    Execute(ast::SimpleCommand),
    Pipe(Vec<JobId>),
    SkipIf(JobId),
    SkipIfNot(JobId),
    Bail(i32),
    Goto(isize),
    WaitFor(Vec<JobId>),
    Launch(JobId),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FdAction {
    Dup(RawFd),
    Dup2(RawFd, RawFd),
    Move(RawFd, RawFd),
    Close(RawFd),
}

pub struct RawCommand {
    executable: CString,
    arguments: Vec<CString>,
}

#[derive(Debug, Clone)]
pub struct Job {
    id: JobId,
    status: JobStatus,

    queue: VecDeque<Action>,
    fd_actions: Vec<FdAction>,
    files: Vec<RawFd>,
    variables: Vec<CString>,
}

#[derive(Debug)]
pub struct ExecutionEnvironment {
    vars: env::Variables,
    running_jobs: HashMap<unistd::Pid, JobId>,
    queued_jobs: Vec<Job>,
    pub fail_fast: bool,
}

fn exec(cmd: &RawCommand, fd_actions: &[FdAction], variables: &[CString]) -> Result<()> {
    for a in fd_actions {
        match a {
            FdAction::Dup(fd) => {
                unistd::dup(*fd).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
            }
            FdAction::Move(from, to) => {
                unistd::dup2(*from, *to).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
                unistd::close(*from).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
            }
            FdAction::Dup2(source, dest) => {
                unistd::dup2(*source, *dest).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
            }
            FdAction::Close(fd) => {
                unistd::close(*fd).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
            }
        }
    }

    if variables.len() > 0 {
        unistd::execve(&cmd.executable, &cmd.arguments, variables)
    } else {
        unistd::execv(&cmd.executable, &cmd.arguments)
    }.context(ErrorKind::ExecFailed)?;
    Ok(())
}

pub fn spawn_raw(
    cmd: &RawCommand,
    fd_actions: &[FdAction],
    variables: &[CString],
) -> Result<unistd::Pid> {
    match unistd::fork().context(ErrorKind::ForkFailed)? {
        unistd::ForkResult::Child => {
            match exec(cmd, fd_actions, variables) {
                Ok(_) => unreachable!(),
                Err(e) => {
                    println!("[rush] failed to start \"{:?}\": {}", cmd.arguments, e);
                    process::exit(1);
                }
            };
        }
        unistd::ForkResult::Parent { child } => Ok(child),
    }
}

impl ExecutionEnvironment {
    pub fn new() -> ExecutionEnvironment {
        ExecutionEnvironment {
            vars: env::Variables::from_env(),
            fail_fast: false,
            running_jobs: HashMap::new(),
            queued_jobs: Vec::new(),
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

    pub fn make_raw_command(&mut self, cmd: &lang::ast::SimpleCommand) -> Result<RawCommand> {
        let mut iter = cmd.arguments.iter();
        let first = iter.next().unwrap().compile(&mut self.vars)?;
        let exe = self.find_executable(&first)?;
        let mut cargs = Vec::with_capacity(cmd.arguments.len());

        cargs.push(CString::new(first).context(ErrorKind::IllegalNullByte)?);
        for x in iter {
            cargs
                .push(CString::new(x.compile(&mut self.vars)?).context(ErrorKind::IllegalNullByte)?);
        }
        Ok(RawCommand {
            executable: CString::new(exe.to_string_lossy().to_string())
                .context(ErrorKind::IllegalNullByte)?,
            arguments: cargs,
        })
    }

    pub fn job<'b>(&'b self, jid: JobId) -> Result<&'b Job> {
        match self.queued_jobs.iter().nth(jid) {
            Some(v) => Ok(v),
            None => Err(ErrorKind::InvalidJobId(jid).into()),
        }
    }

    pub fn job_mut<'b>(&'b mut self, jid: JobId) -> Result<&'b mut Job> {
        match self.queued_jobs.iter_mut().nth(jid) {
            Some(v) => Ok(v),
            None => Err(ErrorKind::InvalidJobId(jid).into()),
        }
    }

    pub fn fork(&mut self, jid: JobId) -> Result<JobId> {
        let new_jid = self.queued_jobs.len();
        let mut new_job = self.job(jid)?.clone();
        new_job.id = new_jid;
        self.queued_jobs.push(new_job);
        Ok(new_jid)
    }

    pub fn schedule(&mut self) -> Result<JobId> {
        let jid = self.queued_jobs.len();

        self.queued_jobs.push(Job {
            id: jid,
            files: Vec::new(),
            status: JobStatus::Sleeping,
            queue: VecDeque::new(),
            fd_actions: Vec::new(),
            variables: Vec::new(),
        });

        Ok(jid)
    }

    pub fn launch_job(&mut self, jid: JobId) -> Result<()> {
        let action = match self.job_mut(jid) {
            Ok(v) => match v.queue.pop_front() {
                Some(v) => v,
                None => {
                    v.status = JobStatus::Finished(0);
                    return Ok(());
                }
            },
            Err(e) => return Err(e),
        };

        match action {
            Action::SkipIf(condition) => {
                self.wait_for(&[condition]);
                let status = self.job(condition)?.status.clone();
                match status {
                    JobStatus::Finished(c) => if c == 0 {
                        self.job_mut(jid)?.queue.pop_front();
                    },
                    _ => (),
                };
            }
            Action::SkipIfNot(condition) => {
                self.wait_for(&[condition]);
                let status = self.job(condition)?.status.clone();
                match status {
                    JobStatus::Finished(c) => if c != 0 {
                        self.job_mut(jid)?.queue.pop_front();
                    },
                    _ => (),
                };
            }
            Action::Bail(c) => {
                self.job_mut(jid)?.status = JobStatus::Finished(c);
                self.job_mut(jid)?.queue.clear();
            }
            Action::Execute(c) => {
                let process = {
                    let command = self.make_raw_command(&c)?;
                    let job = self.job(jid)?;
                    if job.status != JobStatus::Sleeping {
                        return Err(ErrorKind::FailedToRunJob(jid, job.status).into());
                    }

                    let p = spawn_raw(&command, &job.fd_actions, &job.variables)?;
                    p
                };
                self.job_mut(jid)?.status = JobStatus::Running;

                self.running_jobs.insert(process, jid);
            }
            Action::WaitFor(child_jib) => {
                self.wait_for(&child_jib)?;
            }
            Action::Launch(child_jib) => {
                self.launch_job(child_jib)?;
            }
            Action::Pipe(mut jids) => {
                let count = jids.len();
                let mut pipes = Vec::new();
                for _ in 0..(count - 1) {
                    pipes.push(unistd::pipe().context(ErrorKind::PipelineCreationFailed)?);
                }
                let my_actions = self.job(jid)?.fd_actions.clone();

                let mut pipe = 0;
                {
                    let stdout = pipes[0].1;
                    let j = self.job_mut(jids[0])?;
                    j.fd_actions.extend(my_actions.iter());
                    j.fd_actions.push(FdAction::Move(stdout, 1));
                    for (close_stdin, close_stdout) in pipes.iter() {
                        if *close_stdout != stdout {
                            j.fd_actions.push(FdAction::Close(*close_stdout));
                        }
                        j.fd_actions.push(FdAction::Close(*close_stdin));
                    }
                    pipe += 1;
                }

                for jid_idx in 1..(count - 1) {
                    let stdin = pipes[pipe - 1].0;
                    let stdout = pipes[pipe].1;

                    let j = self.job_mut(jids[jid_idx])?;
                    j.fd_actions.extend(my_actions.iter());
                    j.fd_actions.push(FdAction::Move(stdout, 1));
                    j.fd_actions.push(FdAction::Move(stdin, 0));

                    for (close_stdin, close_stdout) in pipes.iter() {
                        if *close_stdout != stdout {
                            j.fd_actions.push(FdAction::Close(*close_stdout));
                        }
                        if *close_stdin != stdin {
                            j.fd_actions.push(FdAction::Close(*close_stdin));
                        }
                    }

                    pipe += 1;
                }
                {
                    let stdin = pipes[count - 2].0;
                    let j = self.job_mut(jids[count - 1])?;
                    j.fd_actions.extend(my_actions.iter());
                    j.fd_actions.push(FdAction::Move(stdin, 0));

                    for (close_stdin, close_stdout) in pipes.iter() {
                        if *close_stdin != stdin {
                            j.fd_actions.push(FdAction::Close(*close_stdin));
                        }
                        j.fd_actions.push(FdAction::Close(*close_stdout));
                    }
                }
                for jid in jids {
                    self.launch_job(jid);
                }

                for pipe in pipes {
                    unistd::close(pipe.0).context(ErrorKind::FailedToClosePipeFile(pipe.0))?;
                    unistd::close(pipe.1).context(ErrorKind::FailedToClosePipeFile(pipe.1))?;
                }
            }

            _ => unimplemented!(),
        }

        Ok(())
    }

    pub fn cleanup(&mut self, pid: unistd::Pid) -> Result<Option<JobId>> {
        match self.running_jobs.get(&pid) {
            Some(jid) => match self.queued_jobs.iter_mut().nth(*jid) {
                Some(v) => {
                    v.status = JobStatus::Sleeping;
                    Ok(Some(*jid))
                }
                None => Err(ErrorKind::InvalidJobId(*jid).into()),
            },
            None => Ok(None),
        }
    }

    pub fn wait_for(&mut self, jobs: &[JobId]) -> Result<()> {
        // it doesn't matter what the handler is doing, but there has to be one for SIGCHLD
        if !traps::is_trapped(signal::Signal::SIGCHLD) {
            traps::trap(signal::Signal::SIGCHLD, traps::Action::NoOp)
                .context(ErrorKind::WaitFailed)?;
        }

        let mut sigs = signal::SigSet::empty();
        sigs.add(signal::Signal::SIGCHLD);
        loop {
            let mut all_finished = true;
            for &jid in jobs {
                loop {
                    match self.job(jid)?.status {
                        JobStatus::Finished(e) => break,
                        JobStatus::Sleeping => if self.job(jid)?.queue.is_empty() {
                            self.job_mut(jid)?.status = JobStatus::Finished(0);
                        } else {
                            self.launch_job(jid)?;
                        },
                        // TODO handle stopped
                        _ => {
                            all_finished = false;
                            break;
                        }
                    };
                }
            }
            if all_finished {
                return Ok(());
            }
            sigs.wait().context(ErrorKind::SigWaitFailed)?;

            loop {
                match wait::wait().context(ErrorKind::WaitFailed)? {
                    wait::WaitStatus::StillAlive => break,
                    wait::WaitStatus::Exited(pid, exit_code) => match self.cleanup(pid)? {
                        Some(finished_jid) => {
                            for &jid in jobs {
                                self.job_mut(finished_jid)?.status = JobStatus::Finished(exit_code);
                            }
                            break;
                        }
                        None => (), // Not one of our processes
                    },
                    _ => unimplemented!(),
                }
            }
        }
        Ok(())
    }

    fn add_command_to_job(&mut self, cmd: ast::Command, job: JobId) -> Result<()> {
        match cmd {
            lang::ast::Command::SimpleCommand(sc) => {
                self.job_mut(job)?.queue.push_back(Action::Execute(sc));
            }

            lang::ast::Command::Group(g) => {
                for c in g.commands {
                    self.add_command_to_job(c, job)?;
                }
            }

            lang::ast::Command::Pipeline(p) => {
                let mut pipe = p;
                let to = self.make_job(pipe.to.clone())?;
                let mut list = vec![to];

                loop {
                    match pipe.from {
                        ast::Command::Pipeline(child_pipe) => {
                            let to = self.make_job(child_pipe.to.clone())?;
                            list.push(to);
                            pipe = child_pipe;
                        }
                        _ => {
                            let from = self.make_job(pipe.from.clone())?;
                            list.push(from);
                            self.job_mut(job)?
                                .queue
                                .push_back(Action::Pipe(list.iter().rev().map(|x| *x).collect()));
                            self.job_mut(job)?.queue.push_back(Action::WaitFor(list));
                            break;
                        }
                    }
                }
            }

            lang::ast::Command::ConditionalPair(c) => {
                let mut cond = c;
                let mut left = self.make_job(cond.left.clone())?;

                loop {
                    match cond.operator {
                        ast::ConditionOperator::OrIf => {
                            self.job_mut(job)?.queue.push_back(Action::SkipIfNot(left));
                        }
                        ast::ConditionOperator::AndIf => {
                            self.job_mut(job)?.queue.push_back(Action::SkipIf(left));
                        }
                    }
                    self.job_mut(job)?.queue.push_back(Action::Bail(1));
                    self.add_command_to_job(cond.right.clone(), job)?;
                    match cond.left {
                        ast::Command::ConditionalPair(child_cond) => {
                            left = self.make_job(child_cond.left.clone())?;
                            cond = child_cond;
                        }
                        _ => {
                            break;
                        }
                    }
                }
            }

            lang::ast::Command::FileRedirect(r) => {
                self.add_command_to_job(r.left.clone(), job)?;
                for redir in r.redirects {
                    match redir.operation {
                        ast::IoOperation::OutputDupFd => {
                            let fd2 = redir.file.compile(&mut self.vars)?;
                            self.job_mut(job)?.fd_actions.insert(
                                0,
                                FdAction::Dup2(fd2.parse().unwrap(), redir.fd.unwrap_or(1)),
                            ); // TODO error handling
                        }
                        _ => unimplemented!(),
                    }
                }
            }
            _ => unimplemented!(),
        };
        Ok(())
    }

    pub fn variables<'a>(&'a self) -> &'a env::Variables {
        &self.vars
    }

    pub fn variables_mut<'a>(&'a mut self) -> &'a mut env::Variables {
        &mut self.vars
    }

    pub fn compile_word<'a>(&mut self, w: &word::Word) -> Result<String> {
        w.compile(&mut self.vars)
    }

    pub fn make_job(&mut self, cmd: ast::Command) -> Result<JobId> {
        let job = self.schedule()?;
        self.add_command_to_job(cmd, job)?;
        Ok(job)
    }

    pub fn spawn<T: Into<ast::Command>>(&mut self, cmd: T) -> Result<JobId> {
        let job = self.make_job(cmd.into())?;
        self.launch_job(job)?;
        Ok(job)
    }

    pub fn run<T: Into<ast::Command>>(&mut self, cmd: T) -> Result<i32> {
        let jid = self.spawn(cmd.into())?;
        self.wait_for(&[jid])?;
        match self.job(jid)?.status {
            JobStatus::Finished(exit_code) => Ok(exit_code),
            _ => unimplemented!(),
        }
    }
}

use env;
use env::traps::trap;
use failure::ResultExt;
use nix;
use nix::sys::signal;
use nix::sys::wait;
use nix::unistd::{close, dup, dup2, execv, execve, fork, ForkResult, Pid};
use nom;
use shell;
use shell::ast;
use shell::parser;
use shell::{Error, ErrorKind, Result};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::env::split_paths;
use std::ffi::CString;
use std::ffi::{OsStr, OsString};
use std::os::unix::io::RawFd;
use std::path;
use std::rc::Rc;
use std::vec::Vec;
pub type JobId = usize;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum JobStatus {
    Running,
    Finished(i32),
    Stopped,
    Sleeping,
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
pub struct Job<'a> {
    id: JobId,
    status: JobStatus,

    queue: VecDeque<ast::SimpleCommand<'a>>,
    dependencies: Vec<JobId>,
    fd_actions: Vec<FdAction>,
    variables: Vec<CString>,
}

#[derive(Debug)]
pub struct ExecutionEnvironment<'a> {
    vars: env::Variables,
    running_jobs: HashMap<Pid, JobId>,
    queued_jobs: Vec<Job<'a>>,
    pub fail_fast: bool,
}

fn exec(cmd: &RawCommand, fd_actions: &[FdAction], variables: &[CString]) -> Result<()> {
    for a in fd_actions {
        match a {
            FdAction::Dup(fd) => {
                dup(*fd).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
            }
            FdAction::Move(from, to) => {
                dup2(*from, *to).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
                close(*from).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
            }
            FdAction::Dup2(source, dest) => {
                dup2(*source, *dest).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
            }
            FdAction::Close(fd) => {
                close(*fd).context(ErrorKind::FdTableMutationFailed(a.clone()))?;
            }
        }
    }

    if variables.len() > 0 {
        execve(&cmd.executable, &cmd.arguments, variables)
    } else {
        execv(&cmd.executable, &cmd.arguments)
    }.context(ErrorKind::ExecFailed)?;
    Ok(())
}

pub fn spawn_raw(cmd: &RawCommand, fd_actions: &[FdAction], variables: &[CString]) -> Result<Pid> {
    match fork().context(ErrorKind::ForkFailed)? {
        ForkResult::Child => {
            match exec(cmd, fd_actions, variables) {
                Ok(_) => (),
                Err(e) => println!("[rush] before exec: {}", e),
            };
            unreachable!()
        }
        ForkResult::Parent { child } => Ok(child),
    }
}

impl<'a> ExecutionEnvironment<'a> {
    pub fn new() -> ExecutionEnvironment<'a> {
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

    pub fn make_raw_command(&mut self, cmd: &shell::ast::SimpleCommand<'a>) -> Result<RawCommand> {
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

    pub fn job<'b>(&'b self, jid: JobId) -> Result<&'b Job<'a>> {
        match self.queued_jobs.iter().nth(jid) {
            Some(v) => Ok(v),
            None => Err(ErrorKind::InvalidJobId(jid).into()),
        }
    }

    pub fn job_mut<'b>(&'b mut self, jid: JobId) -> Result<&'b mut Job<'a>> {
        match self.queued_jobs.iter_mut().nth(jid) {
            Some(v) => Ok(v),
            None => Err(ErrorKind::InvalidJobId(jid).into()),
        }
    }

    pub fn fork(&mut self, jid: JobId) -> Result<JobId> {
        let new_jid = self.queued_jobs.len();
        let new_job = self.job(jid)?.clone();
        self.queued_jobs.push(new_job);
        Ok(new_jid)
    }

    pub fn schedule(&mut self) -> Result<JobId> {
        let jid = self.queued_jobs.len();

        self.queued_jobs.push(Job {
            id: jid,
            status: JobStatus::Sleeping,
            queue: VecDeque::new(),
            fd_actions: Vec::new(),
            variables: Vec::new(),
            dependencies: Vec::new(),
        });

        Ok(jid)
    }

    pub fn launch_job(&mut self, jid: JobId) -> Result<()> {
        let command = match self.job_mut(jid) {
            Ok(v) => v.queue.pop_back().unwrap(), // TODO: check to make sure the queue isn't empty
            Err(e) => return Err(e),
        };

        let raw_command = self.make_raw_command(&command)?;

        let process = {
            let job = self.job_mut(jid)?;
            if job.status != JobStatus::Sleeping {
                return Err(ErrorKind::FailedToRunJob(jid, job.status).into());
            }

            let p = spawn_raw(&raw_command, &job.fd_actions, &job.variables)?;
            job.status = JobStatus::Running;
            p
        };

        self.running_jobs.insert(process, jid);

        Ok(())
    }

    pub fn cleanup(&mut self, pid: Pid) -> Result<Option<JobId>> {
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

    pub fn wait_for(&mut self, jid: JobId) -> Result<i32> {
        // it doesn't matter what the handler is doing, but there has to be one for SIGCHLD
        if !env::traps::is_trapped(signal::Signal::SIGCHLD) {
            env::traps::trap(signal::Signal::SIGCHLD, env::traps::Action::NoOp)
                .context(ErrorKind::WaitFailed)?;
        }

        let mut sigs = signal::SigSet::empty();
        let mut ret = None;
        sigs.add(signal::Signal::SIGCHLD);
        loop {
            let sig = sigs.wait().context(ErrorKind::WaitFailed)?;

            loop {
                match wait::wait().context(ErrorKind::WaitFailed)? {
                    wait::WaitStatus::StillAlive => break,
                    wait::WaitStatus::Exited(pid, exit_code) => match self.cleanup(pid)? {
                        Some(finished_jid) => {
                            if finished_jid == jid {
                                ret = Some(exit_code);
                            } else {
                                self.job_mut(finished_jid).unwrap().status =
                                    JobStatus::Finished(exit_code);
                            }
                        }
                        None => (), // Not one of our processes
                    },
                    _ => unimplemented!(),
                }

                if let Some(exit_code) = ret {
                    return Ok(exit_code);
                }
            }
        }
    }

    fn add_command_to_job(&mut self, cmd: ast::Command<'a>, job: JobId) -> Result<()> {
        match cmd {
            shell::ast::Command::SimpleCommand(sc) => {
                self.job_mut(job).unwrap().queue.push_back(sc);
            }

            shell::ast::Command::Group(g) => {
                for c in g.commands {
                    self.add_command_to_job(c, job)?;
                }
            }
            _ => unimplemented!(),
        };
        Ok(())
    }

    pub fn make_job(&mut self, cmd: ast::Command<'a>) -> Result<JobId> {
        let job = self.schedule()?;
        self.add_command_to_job(cmd, job)?;
        Ok(job)
    }

    pub fn spawn(&mut self, cmd: ast::Command<'a>) -> Result<JobId> {
        let job = self.make_job(cmd)?;
        self.launch_job(job)?;
        Ok(job)
    }

    pub fn run(&mut self, cmd: ast::Command<'a>) -> Result<i32> {
        let jid = self.spawn(cmd)?;
        self.wait_for(jid)
    }

    pub fn run_str(&mut self, s: &'a str) -> Result<i32> {
        let cmd = parser::commandline(nom::types::CompleteStr(s)).unwrap().1;
        self.run(cmd)
    }
}

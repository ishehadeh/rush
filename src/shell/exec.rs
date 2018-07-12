use env;
use env::traps;
use failure::ResultExt;
use nix::sys::signal;
use nix::sys::wait;
use nix::unistd;
use shell;
use shell::ast;
use shell::{Error, ErrorKind, Result};
use std::collections::{HashMap, VecDeque};
use std::env::split_paths;
use std::ffi::CString;
use std::ffi::{OsStr, OsString};
use std::os::unix::io::RawFd;
use std::path;
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
    Pipe(JobId, JobId),
    SkipIf(ast::SimpleCommand),
    SkipIfNot(ast::SimpleCommand),
    Goto(isize),
    WaitFor(JobId),
    WaitAll,
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
    dependancies: Vec<JobId>,
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
                Ok(_) => (),
                Err(e) => println!("[rush] before exec: {}", e),
            };
            unreachable!()
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

    pub fn make_raw_command(&mut self, cmd: &shell::ast::SimpleCommand) -> Result<RawCommand> {
        let mut iter = cmd.arguments.iter();
        let first = iter.next().unwrap().compile(&mut self.vars)?;
        let exe = self.find_executable(&first)?;
        let mut cargs = Vec::with_capacity(cmd.arguments.len());

        cargs.push(CString::new(first).context(ErrorKind::IllegalNullByte)?);
        for x in iter {
            cargs.push(
                CString::new(x.compile(&mut self.vars)?).context(ErrorKind::IllegalNullByte)?,
            );
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
            dependancies: Vec::new(),
        });

        Ok(jid)
    }

    pub fn launch_job(&mut self, jid: JobId) -> Result<()> {
        let action = match self.job_mut(jid) {
            Ok(v) => match v.queue.pop_back() {
                Some(v) => v,
                None => return Err(ErrorKind::FailedToRunJob(jid, v.status).into()),
            },
            Err(e) => return Err(e),
        };

        match action {
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
            Action::Pipe(from_jid, to_jid) => {
                let (stdin, stdout) = unistd::pipe().context(ErrorKind::PipelineCreationFailed)?;
                {
                    let from = self.job_mut(from_jid)?;
                    from.fd_actions.push(FdAction::Move(stdout, 1));
                    from.fd_actions.push(FdAction::Close(stdin))
                }
                {
                    let to = self.job_mut(to_jid)?;
                    to.fd_actions.push(FdAction::Move(stdin, 0));
                    to.fd_actions.push(FdAction::Close(stdout));
                }
                {
                    let deps = self.job(from_jid)?.dependancies.clone();
                    for dep in deps {
                        let to = self.job_mut(dep)?;
                        to.fd_actions.push(FdAction::Move(stdout, 1));
                        to.fd_actions.push(FdAction::Close(stdin))
                    }
                }
                {
                    let deps = self.job(to_jid)?.dependancies.clone();
                    for dep in deps {
                        let to = self.job_mut(dep)?;
                        to.fd_actions.insert(0, FdAction::Move(stdin, 0));
                        to.fd_actions.insert(0, FdAction::Close(stdout))
                    }
                }

                self.launch_job(from_jid)?;
                self.launch_job(to_jid)?;

                unistd::close(stdout).context(ErrorKind::FailedToClosePipeFile(stdout))?;
                unistd::close(stdin).context(ErrorKind::FailedToClosePipeFile(stdin))?;
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

    pub fn wait_for(&mut self, jid: JobId) -> Result<i32> {
        // it doesn't matter what the handler is doing, but there has to be one for SIGCHLD
        if !traps::is_trapped(signal::Signal::SIGCHLD) {
            traps::trap(signal::Signal::SIGCHLD, traps::Action::NoOp)
                .context(ErrorKind::WaitFailed)?;
        }

        let mut sigs = signal::SigSet::empty();
        let mut ret = None;
        sigs.add(signal::Signal::SIGCHLD);
        loop {
            sigs.wait().context(ErrorKind::WaitFailed)?;

            loop {
                match wait::wait().context(ErrorKind::WaitFailed)? {
                    wait::WaitStatus::StillAlive => break,
                    wait::WaitStatus::Exited(pid, exit_code) => match self.cleanup(pid)? {
                        Some(finished_jid) => {
                            self.job_mut(finished_jid)?.status = JobStatus::Finished(exit_code);
                            let job = self.job_mut(jid)?;
                            let is_dep = job
                                .dependancies
                                .iter()
                                .position(|&x| x == finished_jid)
                                .map(|e| job.dependancies.remove(e))
                                .is_some();

                            if job.dependancies.len() == 0 && !is_dep {
                                if finished_jid == jid {
                                    ret = Some(exit_code);
                                }
                            } else if is_dep && job.status == JobStatus::Sleeping {
                                ret = Some(exit_code);
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

    fn add_command_to_job(&mut self, cmd: ast::Command, job: JobId) -> Result<()> {
        match cmd {
            shell::ast::Command::SimpleCommand(sc) => {
                self.job_mut(job)?.queue.push_back(Action::Execute(sc));
            }

            shell::ast::Command::Group(g) => {
                for c in g.commands {
                    self.add_command_to_job(c, job)?;
                }
            }

            shell::ast::Command::Pipeline(p) => {
                let from = self.fork(job)?;
                let to = self.fork(job)?;
                self.add_command_to_job(p.from.clone(), from)?;
                self.add_command_to_job(p.to.clone(), to)?;
                self.job_mut(job)?.queue.push_back(Action::Pipe(from, to));
                self.job_mut(job)?.dependancies.extend(&[from, to]);
            }

            shell::ast::Command::FileRedirect(r) => {
                self.add_command_to_job(r.left.clone(), job)?;
                for redir in r.redirects {
                    match redir.operation {
                        ast::IoOperation::OutputDupFd => {
                            let fd2 = redir.file.compile(&mut self.vars)?;
                            self.job_mut(job)?
                                .fd_actions
                                .push(FdAction::Dup2(redir.fd.unwrap_or(1), fd2.parse().unwrap())); // TODO error handling
                        }
                        _ => unimplemented!(),
                    }
                }
            }
            _ => unimplemented!(),
        };
        Ok(())
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
        self.wait_for(jid)
    }
}

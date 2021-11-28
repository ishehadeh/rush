use nix::{
    self,
    unistd::{ForkResult, Pid},
};
use std::{
    env,
    ffi::CString,
    fmt,
    path::{Path, PathBuf},
    process::exit,
};

/// An error that occurs in a subprocess during setup, before `exec` is called.
#[derive(Clone, Debug, PartialEq)]
pub enum SubprocessSetupError {
    /// Failed to close a file descriptor
    CloseFailed { source: nix::Error, fd: i32 },

    /// Failed to redirect a file descriptor to another
    DupFailed {
        source: nix::Error,
        oldfd: i32,
        newfd: i32,
    },

    /// Context around a failed call to open()
    OpenFailed {
        source: nix::Error,
        file: PathBuf,
        flags: nix::fcntl::OFlag,
        permissions: nix::sys::stat::Mode,
    },

    /// Failed to open and map a file descriptor to it.
    ///
    /// The source of this error should be Self::OpenFailed or Self::DupFailed
    OpenAndDupFailed {
        source: Box<Self>,
        file: PathBuf,
        mode: OpenMode,
        fd: i32,
    },

    /// An argument contains a null character
    ArgContainsNull { arg_number: usize, arg: String },

    /// Failed to chdir to the process' working directory
    SetWorkDirFailed { source: nix::Error, path: PathBuf },

    /// A call to exec() failed
    ExecFailed {
        args: Vec<String>,
        executable: String,
        source: nix::Error,
    },
}

impl fmt::Display for SubprocessSetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CloseFailed { source, fd } => write!(f, "failed to close fd {}: {}", fd, source),
            Self::DupFailed {
                source,
                newfd,
                oldfd,
            } => write!(f, "failed to dup fd {} to {}: {}", newfd, oldfd, source),
            Self::OpenFailed { source, file, .. } => {
                write!(f, "could not open {:?}: {}", file, source)
            }
            Self::OpenAndDupFailed {
                source,
                file,
                mode,
                fd,
            } => {
                let action = match mode {
                    OpenMode::Read => "reading",
                    _ => "writing",
                };
                write!(
                    f,
                    "failed to open {:?} for {} on file descriptor {}: {}",
                    file, action, fd, source
                )
            }
            Self::ArgContainsNull { arg_number, arg } => {
                write!(
                    f,
                    "cannot exec, arg {} contains a null byte: {:?}",
                    arg_number, arg
                )
            }
            Self::ExecFailed {
                source, executable, ..
            } => {
                write!(f, "exec() failed for {:?}: {}", executable, source)
            }
            Self::SetWorkDirFailed { source, path } => {
                write!(
                    f,
                    "failed to set process working directory to {:?}: {}",
                    path, source
                )
            }
        }
    }
}

impl std::error::Error for SubprocessSetupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CloseFailed { source, .. } => Some(source),
            Self::DupFailed { source, .. } => Some(source),
            Self::OpenFailed { source, .. } => Some(source),

            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SpawnError {
    ForkFailed { source: nix::Error },
}

impl fmt::Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForkFailed { source } => write!(f, "fork failed: {}", source),
        }
    }
}
impl std::error::Error for SpawnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ForkFailed { source, .. } => Some(source),
        }
    }
}

/// Operation to apply to a file descriptor
///
/// This enum is used to describe what should be done with open file descriptors after calling fork()
#[derive(Clone, Debug)]
pub enum FdOp {
    /// Redirect another file descriptor to this one
    ///
    /// NOTE: This is the _target_ of the redirect, the left one in the tuple in FdOp is the _source_
    Redirect(i32),

    /// Open a new file and redirect the file descriptor to it
    Open(PathBuf, OpenMode),

    /// Close the file descriptor
    Close,
}

/// Modes a file can be opened in, these are pretty specialized to shells '>>', '<', etc operators
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum OpenMode {
    /// Open a file for reading
    Read,

    /// Open a file for writing, create it if it doesnt exist
    Write,

    /// Open a file for writing, appending to the content in the file. If the file does not exist create it
    Append,
}

/// Description of a process to be spawned
pub struct ProcessOptions {
    /// Arguments passed to the executable
    args: Vec<String>,

    /// Process working directory, `None` means inherit from parent process
    wd: Option<PathBuf>,

    /// absolute path to the exeutable file
    executable: String,

    /// *Additional* environment variables to be set for this process, it will inherit all variables from the current process
    env: Vec<(String, String)>,

    /// List of file descriptors and actions to perform on them
    fd: Vec<(i32, FdOp)>,
}

impl ProcessOptions {
    pub fn new(exe: &str) -> ProcessOptions {
        ProcessOptions {
            executable: exe.to_string(),
            args: vec![],
            env: vec![],
            fd: vec![],
            wd: None,
        }
    }
    pub fn arg(&mut self, arg: &str) -> &mut ProcessOptions {
        self.args.push(arg.into());
        self
    }

    pub fn env(&mut self, k: &str, v: &str) -> &mut ProcessOptions {
        self.env.push((k.into(), v.into()));
        self
    }

    pub fn work_dir<P: Into<PathBuf>>(&mut self, dir: P) -> &mut ProcessOptions {
        self.wd = Some(dir.into());
        self
    }

    pub fn read<I: Into<PathBuf>>(&mut self, fd: i32, file: I) -> &mut ProcessOptions {
        self.fd.push((fd, FdOp::Open(file.into(), OpenMode::Read)));
        self
    }

    pub fn write<I: Into<PathBuf>>(&mut self, fd: i32, file: I) -> &mut ProcessOptions {
        self.fd.push((fd, FdOp::Open(file.into(), OpenMode::Write)));
        self
    }

    pub fn append<I: Into<PathBuf>>(&mut self, fd: i32, file: I) -> &mut ProcessOptions {
        self.fd
            .push((fd, FdOp::Open(file.into(), OpenMode::Append)));
        self
    }

    pub fn close(&mut self, fd: i32) -> &mut ProcessOptions {
        self.fd.push((fd, FdOp::Close));
        self
    }

    pub fn redirect(&mut self, source_fd: i32, target_fd: i32) -> &mut ProcessOptions {
        self.fd.push((source_fd, FdOp::Redirect(target_fd)));
        self
    }

    pub fn spawn(&self) -> Result<Pid, SpawnError> {
        match nix::unistd::fork() {
            Err(source) => Err(SpawnError::ForkFailed { source }),
            Ok(ForkResult::Child) => {
                if let Err(e) = setup_subprocess(self) {
                    eprintln!("could not spawn {:?}: {}", self.executable, e);
                    exit(1);
                }

                if let Err(e) = exec_subprocess(&self.executable, &self.args) {
                    // don't mention the executable here because its in the error message
                    eprintln!("{}", e);
                    exit(1);
                }

                unreachable!();
            }

            Ok(ForkResult::Parent { child }) => Ok(child),
        }
    }
}

/// Wrapper around dup2 that maps the error to SubprocessSetupError
fn dup(oldfd: i32, newfd: i32) -> Result<(), SubprocessSetupError> {
    match nix::unistd::dup2(oldfd, newfd) {
        Err(source) => Err(SubprocessSetupError::DupFailed {
            oldfd,
            newfd,
            source,
        }),
        _ => Ok(()),
    }
}

/// Wrapper around close that maps the error to SubprocessSetupError
fn close(fd: i32) -> Result<(), SubprocessSetupError> {
    nix::unistd::close(fd).map_err(|source| SubprocessSetupError::CloseFailed { fd, source })
}

/// Open a file with access 0644 and flags determined by `mode`, return the new file descriptor
///
/// OpenMode map:
/// - Read: O_RDONLY
/// - Write: O_WRONLY | O_CREAT | O_TRUNC
/// - Append: O_WRONLY | O_CREAT | O_APPEND
fn open<P: AsRef<Path>>(path: P, mode: OpenMode) -> Result<i32, SubprocessSetupError> {
    use nix::fcntl::OFlag;
    use nix::sys::stat::Mode;

    // permission 0644/-rw-r--r--, readable by everyone, writable by owner only
    let permissions = Mode::S_IRUSR | Mode::S_IWUSR | Mode::S_IRGRP | Mode::S_IROTH;

    let flags = match mode {
        OpenMode::Read => OFlag::O_RDONLY,
        OpenMode::Write => OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_TRUNC,
        OpenMode::Append => OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_APPEND,
    };

    nix::fcntl::open(path.as_ref(), flags, permissions).map_err(|source| {
        return SubprocessSetupError::OpenFailed {
            file: path.as_ref().to_path_buf(),
            flags,
            permissions,
            source,
        };
    })
}

/// Open a file with open() and map it to another file descriptor with dup()
fn open_and_dup<P: AsRef<Path>>(
    path: P,
    mode: OpenMode,
    fd: i32,
) -> Result<(), SubprocessSetupError> {
    let oldfd = open(path, mode)?;
    dup(oldfd, fd)
}

fn setup_subprocess(opts: &ProcessOptions) -> Result<(), SubprocessSetupError> {
    for (key, value) in &opts.env {
        env::set_var(key, value);
    }

    for (fd, op) in &opts.fd {
        match op {
            FdOp::Close => close(*fd)?,
            FdOp::Redirect(newfd) => dup(*fd, *newfd)?,
            FdOp::Open(path, mode) => open_and_dup(path, *mode, *fd).map_err(|source| {
                SubprocessSetupError::OpenAndDupFailed {
                    file: path.clone(),
                    fd: *fd,
                    mode: *mode,
                    source: Box::new(source),
                }
            })?,
        }
    }

    if let Some(dir) = &opts.wd {
        nix::unistd::chdir(dir).map_err(|source| SubprocessSetupError::SetWorkDirFailed {
            source,
            path: dir.clone(),
        })?;
    }

    Ok(())
}

fn exec_subprocess(exe: &str, args: &[String]) -> Result<(), SubprocessSetupError> {
    let c_exe =
        CString::new(exe.as_bytes()).map_err(|_| SubprocessSetupError::ArgContainsNull {
            arg_number: 0,
            arg: exe.to_string(),
        })?;

    let mut c_args = Vec::with_capacity(args.len());
    for (i, arg) in args.iter().enumerate() {
        c_args.push(CString::new(arg.as_bytes()).map_err(|_| {
            SubprocessSetupError::ArgContainsNull {
                arg_number: i + 1,
                arg: arg.clone(),
            }
        })?);
    }

    nix::unistd::execve(&c_exe, &c_args, &[]).map_err(|source| {
        SubprocessSetupError::ExecFailed {
            source,
            executable: exe.to_string(),
            args: args.to_owned(),
        }
    })?;

    unreachable!();
}

#[cfg(test)]
mod test {
    use crate::{
        jobs::spawn::{OpenMode, ProcessOptions, SubprocessSetupError},
        test_util::forks,
    };
    use nix::{
        sys::wait::{waitpid, WaitStatus},
        unistd::Pid,
    };
    use std::{
        collections::HashSet,
        fs::File,
        io::{self, Read},
        path::PathBuf,
    };

    // first couple tests here are just sanity tests for the basic operations
    // they also check the errors are properly mapped to SubprocessSetupError
    #[test]
    fn setup_subprocess_open_close() {
        let fd = super::open("test/data/hello.txt", OpenMode::Read).unwrap();
        let mut buf = [0u8; 5];
        nix::unistd::read(fd, &mut buf).unwrap();
        assert_eq!(&buf, b"hello");

        super::close(fd).unwrap();
        match super::close(fd).unwrap_err() {
            SubprocessSetupError::CloseFailed {
                fd: close_err_fd, ..
            } => {
                assert_eq!(close_err_fd, fd);
            }
            e => panic!(
                "expected SubprocessSetupError::CloseFailed from close() got {:?}",
                e
            ),
        }

        match super::open("test/data/DOES NOT EXIST", OpenMode::Read).unwrap_err() {
            SubprocessSetupError::OpenFailed {
                file,
                flags,
                permissions,
                ..
            } => {
                assert_eq!(file, PathBuf::from("test/data/DOES NOT EXIST"));
                assert_eq!(flags, nix::fcntl::OFlag::O_RDONLY);
                assert_eq!(permissions.bits(), 0o644);
            }
            e => panic!(
                "expected SubprocessSetupError::OpenFailed from open() got {:?}",
                e
            ),
        }
    }

    #[test]
    fn setup_process_dup_close() {
        let fd = super::open("test/data/hello.txt", OpenMode::Read).unwrap();
        let newfd = 11;
        super::dup(fd, newfd).unwrap();

        let mut buf = [0u8; 5];
        nix::unistd::read(newfd, &mut buf).unwrap();
        assert_eq!(&buf, b"hello");

        super::close(newfd).unwrap();
        super::close(fd).unwrap();

        match super::dup(-1, 12).unwrap_err() {
            SubprocessSetupError::DupFailed { oldfd, newfd, .. } => {
                assert_eq!(oldfd, -1);
                assert_eq!(newfd, 12);
            }
            e => panic!(
                "expected SubprocessSetupError::DupFailed from dup() got {:?}",
                e
            ),
        }
    }

    #[test]
    fn spawn_printf() {
        forks!();

        let out_file = PathBuf::from("test/data/spawn_printf-out.txt");
        match std::fs::remove_file(&out_file) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(err) => panic!("failed to remove file: {}", err),
        }

        let pid = ProcessOptions::new("/usr/bin/printf")
            .arg("%s")
            .arg("hello world")
            .redirect(1, 2)
            .close(2)
            .write(1, &out_file)
            .spawn()
            .expect("spawn failed");
        waitpid(pid, None).expect("wait for printf failed");

        let mut content = String::new();
        File::open(&out_file)
            .expect("failed to open file")
            .read_to_string(&mut content)
            .expect("failed to read file");
        assert_eq!(content, "hello world");
    }

    #[test]
    fn spawn_pipe() {
        forks!();

        let out_file = PathBuf::from("test/data/spawn_pipe-out.txt");
        match std::fs::remove_file(&out_file) {
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(err) => panic!("failed to remove file: {}", err),
        }

        let (infd, outfd) = nix::unistd::pipe().expect("failed to create pipe");
        let revpid = ProcessOptions::new("/usr/bin/rev")
            .redirect(infd, 0)
            .write(1, &out_file)
            .close(outfd)
            .close(infd)
            .spawn()
            .expect("failed to spawn rev");
        let printfpid = ProcessOptions::new("/usr/bin/printf")
            .arg("%s")
            .arg("hello")
            .redirect(outfd, 1)
            .close(outfd)
            .close(infd)
            .spawn()
            .expect("failed to spawn printf");

        nix::unistd::close(infd).expect("failed to close pipe input in parent");
        nix::unistd::close(outfd).expect("failed to close pipe output in parent");

        let mut waitlist: HashSet<Pid> = HashSet::from([revpid, printfpid]);
        while !waitlist.is_empty() {
            let waitstatus = waitpid(None, None).expect("waitpid() failed");
            if let WaitStatus::Exited(pid, status) = waitstatus {
                if waitlist.contains(&pid) {
                    if status != 0 {
                        panic!(
                            "child process {} exited with non-zero exit code: {}",
                            pid, status
                        );
                    }

                    waitlist.remove(&pid);
                }
            }
        }

        let mut content = String::new();
        File::open(&out_file)
            .expect("failed to open file")
            .read_to_string(&mut content)
            .expect("failed to read file");
        assert_eq!(content, "olleh");
    }
}

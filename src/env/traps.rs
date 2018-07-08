use nix;
use nix::sys::signal;
pub use nix::sys::signal::Signal;
use std::os::raw::c_int;
use std::slice;
use std::sync::Mutex;

lazy_static! {
    static ref SIGNAL_ACTION_TRAP: signal::SigAction = {
        signal::SigAction::new(
            signal::SigHandler::Handler(__rush_global_signal_handler),
            signal::SaFlags::empty(),
            signal::SigSet::empty(),
        )
    };
}

lazy_static! {
    static ref SIGNAL_ACTION_DEFAULT: signal::SigAction = {
        signal::SigAction::new(
            signal::SigHandler::SigDfl,
            signal::SaFlags::empty(),
            signal::SigSet::empty(),
        )
    };
}

lazy_static! {
    static ref GLOBAL_TRAPS: Mutex<Traps> = { Mutex::new(Traps::new()) };
}

pub struct Traps([Action; 31]);
pub type TrapIter<'a> = slice::Iter<'a, Action>;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Default,
    Eval(String),
}

pub fn trap(sig: Signal, s: String) -> nix::Result<()> {
    let mut mut_traps = GLOBAL_TRAPS.lock().unwrap();
    mut_traps.set(sig, Action::Eval(s));
    unsafe { signal::sigaction(sig, &*SIGNAL_ACTION_TRAP) }.map(|_| ())
}

pub fn trap_s<T: AsRef<str>>(sig: T, s: String) -> nix::Result<()> {
    trap(
        match parse_signal(sig) {
            Some(v) => v,
            None => return Err(nix::Error::UnsupportedOperation),
        },
        s,
    )
}

pub fn release(sig: Signal) -> nix::Result<()> {
    let mut mut_traps = GLOBAL_TRAPS.lock().unwrap();
    mut_traps.set(sig, Action::Default);
    unsafe { signal::sigaction(sig, &*SIGNAL_ACTION_DEFAULT) }.map(|_| ())
}

extern "C" fn __rush_global_signal_handler(sig: c_int) {
    let traps = GLOBAL_TRAPS.lock().unwrap();
    match traps.get_by_id(sig) {
        Action::Default => (),
        Action::Eval(ref s) => {
            println!("\n==> Signal handler for \"{}\"", s);
            unimplemented!();
        }
    }
}

pub fn parse_signal<T: AsRef<str>>(s: T) -> Option<Signal> {
    Some(match s
        .as_ref()
        .to_ascii_uppercase()
        .trim()
        .trim_left_matches("SIG")
    {
        "1" | "HUP" => Signal::SIGHUP,
        "2" | "INT" => Signal::SIGINT,
        "3" | "QUIT" => Signal::SIGQUIT,
        "4" | "ILL" => Signal::SIGILL,
        "5" | "TRAP" => Signal::SIGTRAP,
        "6" | "ABRT" => Signal::SIGABRT,
        "7" | "BUS" => Signal::SIGBUS,
        "8" | "FPE" => Signal::SIGFPE,
        "9" | "KILL" => Signal::SIGKILL,
        "10" | "USR1" => Signal::SIGUSR1,
        "11" | "SEGV" => Signal::SIGSEGV,
        "12" | "USR2" => Signal::SIGUSR2,
        "13" | "PIPE" => Signal::SIGPIPE,
        "14" | "ALRM" => Signal::SIGALRM,
        "15" | "TERM" => Signal::SIGTERM,
        "16" | "STKFLT" => Signal::SIGSTKFLT,
        "17" | "CHLD" => Signal::SIGCHLD,
        "18" | "CONT" => Signal::SIGCONT,
        "19" | "STOP" => Signal::SIGSTOP,
        "20" | "TSTP" => Signal::SIGTSTP,
        "21" | "TTIN" => Signal::SIGTTIN,
        "22" | "TTOU" => Signal::SIGTTOU,
        "23" | "URG" => Signal::SIGURG,
        "24" | "XCPU" => Signal::SIGXCPU,
        "25" | "XFSZ" => Signal::SIGXFSZ,
        "26" | "VTALRM" => Signal::SIGVTALRM,
        "27" | "PROF" => Signal::SIGPROF,
        "28" | "WINCH" => Signal::SIGWINCH,
        "29" | "IO" => Signal::SIGIO,
        "30" | "PWR" => Signal::SIGPWR,
        "31" | "SYS" => Signal::SIGSYS,
        _ => return None,
    })
}

impl Traps {
    pub fn new() -> Traps {
        Traps::default()
    }

    pub fn set(&mut self, sig: Signal, act: Action) {
        self.0[sig as usize] = act;
    }

    pub fn set_from_string<T: AsRef<str>>(&mut self, sig: T, act: Action) -> Option<Signal> {
        match parse_signal(sig) {
            Some(v) => {
                self.set(v, act);
                Some(v)
            }
            None => None,
        }
    }

    pub fn get<'a>(&'a self, sig: Signal) -> &'a Action {
        &self.0[sig as usize]
    }

    pub fn get_by_id<'a>(&'a self, sig: c_int) -> &'a Action {
        &self.0[sig as usize]
    }

    pub fn iter<'a>(&'a self) -> TrapIter<'a> {
        self.0.iter()
    }
}

impl Default for Action {
    fn default() -> Action {
        Action::Default
    }
}

impl Default for Traps {
    fn default() -> Traps {
        Traps([
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
            Action::Default,
        ])
    }
}

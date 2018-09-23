use nix;
use nix::sys::signal;
pub use nix::sys::signal::Signal;
use std::collections::HashMap;
use std::os::raw::c_int;
use std::slice;
use std::sync::RwLock;

lazy_static! {
    static ref GLOBAL_TRAPS: RwLock<Traps> = { RwLock::new(Traps::with_capacity(31)) };
}

pub type LineFn = Box<FnMut() + Send + Sync + 'static>;
pub type Traps = HashMap<Signal, Vec<Action>>;
pub type TrapIter<'a> = slice::Iter<'a, Action>;

pub enum Action {
    NoOp,
    Eval(String),
    Func(LineFn),
}

pub fn trap(sig: Signal, a: Action) -> nix::Result<()> {
    let mut mut_traps = GLOBAL_TRAPS.write().unwrap();
    match mut_traps.get_mut(&sig) {
        Some(v) => {
            v.push(a);
            return Ok(());
        }
        None => unsafe {
            signal::sigaction(
                sig,
                &signal::SigAction::new(
                    signal::SigHandler::Handler(__rush_global_signal_handler),
                    signal::SaFlags::empty(),
                    signal::SigSet::empty(),
                ),
            )
        }.map(|_| ())?,
    };
    mut_traps.insert(sig, vec![a]);
    Ok(())
}

pub fn trap_s<T: AsRef<str>>(sig: T, a: Action) -> nix::Result<()> {
    trap(
        match parse_signal(sig) {
            Some(v) => v,
            None => return Err(nix::Error::UnsupportedOperation),
        },
        a,
    )
}

pub fn release(sig: Signal) -> nix::Result<()> {
    let mut mut_traps = GLOBAL_TRAPS.write().unwrap();
    mut_traps.remove(&sig);
    unsafe {
        signal::sigaction(
            sig,
            &signal::SigAction::new(
                signal::SigHandler::SigDfl,
                signal::SaFlags::empty(),
                signal::SigSet::empty(),
            ),
        )
    }.map(|_| ())
}

pub fn is_trapped(sig: Signal) -> bool {
    return GLOBAL_TRAPS.read().unwrap().contains_key(&sig);
}

extern "C" fn __rush_global_signal_handler(sig: c_int) {
    let mut traps = GLOBAL_TRAPS.write().unwrap();
    match traps.get_mut(&(Signal::from_c_int(sig).unwrap())) {
        Some(actions) => for action in actions {
            match action {
                Action::Eval(ref s) => {
                    println!("\n==> Signal handler for \"{}\"", s);
                    unimplemented!();
                }
                Action::Func(ref mut f) => f(),
                Action::NoOp => (),
            }
        },
        None => (),
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

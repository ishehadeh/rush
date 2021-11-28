//! test_utils provides extra helpers for `rush` unit tests
//!
//! At the moment the main utility this module provides is the `forks!()` macro.
//! When this macro is placed at the top of a test it ensures the test is not run at the same
//! time as any other `forks!()` test, and all subprocesses will be repeated at the end of the test, even on panic

use std::{
    mem::MaybeUninit,
    sync::{Mutex, MutexGuard, Once, PoisonError},
};

/// Wrapper around an static MutexGuard that should be held for the duration of a test
pub struct SerialTestGuard {
    _guard: MutexGuard<'static, ()>,
}

/// block until a SerialTestGuard can be obtained.
///
/// Generally the serial!() macro is prefered to calling the function manually.
pub fn begin_serial_test() -> SerialTestGuard {
    static mut MUTEX: MaybeUninit<Mutex<()>> = MaybeUninit::uninit();
    static ONCE: Once = Once::new();

    ONCE.call_once(|| unsafe {
        MUTEX.write(Mutex::new(()));
    });

    let guard = unsafe { MUTEX.assume_init_ref() }
        .lock()
        .unwrap_or_else(PoisonError::into_inner); // poisoned locks are ok, since we know the test exited if it panicked

    SerialTestGuard { _guard: guard }
}

/// Guard that ensures no other code marked forks!() will be run at the same time.
///
/// The guard lasts from the call to forks!() to end of scope
macro_rules! forks {
    () => {
        let _serial_test_guard = crate::test_util::begin_serial_test();
        let _reaper_guard = crate::test_util::ReaperGuard;
    };
}

pub(crate) use forks;

/// Guard that reaps (i.e. waits) for all this processes children when it is dropped
///
/// ReaperGuard is useful for unit tests which call `fork()`, in case the test panics and leaves processes sitting in the background
pub struct ReaperGuard;

impl Drop for ReaperGuard {
    fn drop(&mut self) {
        loop {
            match nix::sys::wait::wait() {
                // if there are no more children left than break
                Err(nix::Error::Sys(nix::errno::Errno::ECHILD)) => break,
                Err(e) => panic!("ReaperGuard: failed to wait for child processes: {}", e),
                Ok(_) => (),
            }
        }
    }
}

mod test {
    use crate::test_util::*;
    use nix::{
        errno::Errno,
        sys::wait::wait,
        unistd::{fork, ForkResult},
    };
    use std::process::exit;

    #[test]
    fn reaper_guard() {
        forks!();

        {
            let _reaper = ReaperGuard;
            if let ForkResult::Child = fork().expect("fork failed") {
                exit(0)
            }
            // _reaper should be dropped here and clean up the child process
        }

        assert_eq!(wait(), Err(nix::Error::Sys(Errno::ECHILD)));
    }
}

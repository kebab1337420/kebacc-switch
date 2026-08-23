//! How this crate starts other processes on Windows without putting a console
//! window on the user's screen — and without holding a terminal hostage.
//!
//! Two different flags, for two different jobs, and the reason there are two is
//! the thing that bit us: a process started with `DETACHED_PROCESS` has no
//! console at all, so when *it* starts a console program - `git`, `icacls` -
//! Windows allocates a fresh console for that child and draws a window for it.
//! The detached refresh the status line spawns runs `git status`, and the status
//! line is drawn constantly, so that is a window flashing up over whatever the
//! user is doing, several times a minute. The mid-task hook has the same shape:
//! it spawns a detached `auto`, and switching an account writes credentials,
//! which runs `icacls`.
//!
//! So: `detach` for the background copies of ourselves, `hidden` for every
//! external command, and `hidden` is not optional even inside a process that is
//! already detached.
//!
//! Neither flag is enough on its own, though. We run as a hook: Claude Code
//! starts us with our stdout on a pipe and then reads that pipe until it sees
//! end of file. Rust always spawns with `bInheritHandles = TRUE`, and
//! `Stdio::null` only swaps the three handles the child *starts* with — the
//! parent's own inheritable handles are still duplicated into it. So the
//! detached watcher, which lives for hours, was holding a copy of the hook's
//! stdout. The pipe never closed, the read never ended, and the terminal sat
//! there with a tool call that never came back. `spawn_detached` closes that:
//! it makes our standard handles non-inheritable for the length of the spawn,
//! so the background copy starts with none of our pipes.

use std::process::{Child, Command};

/// Run this child with no console window of its own. For external commands.
#[cfg(windows)]
pub fn hidden(command: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(not(windows))]
pub fn hidden(command: &mut Command) -> &mut Command {
    command
}

/// Cut this child loose: it outlives us, it holds no console, and closing the
/// terminal does not take it with it. For the background copies of ourselves.
#[cfg(windows)]
pub fn detach(command: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
}

#[cfg(not(windows))]
pub fn detach(command: &mut Command) -> &mut Command {
    command
}

/// Start a background copy of ourselves that inherits nothing from us.
///
/// Callers set the flags with [`detach`] and point all three stdio at
/// `Stdio::null` first; this only adds the part `Command` cannot express. The
/// child must not hold our stdout, because whoever is reading that pipe — a
/// hook waiting on us, a status line being drawn — waits for every copy of the
/// write end to close, and a watcher that lives twelve hours never closes it.
pub fn spawn_detached(command: &mut Command) -> std::io::Result<Child> {
    #[cfg(windows)]
    {
        let _sealed = windows::Sealed::new();
        command.spawn()
    }
    #[cfg(not(windows))]
    {
        command.spawn()
    }
}

#[cfg(windows)]
mod windows {
    use std::io::{stderr, stdin, stdout};
    use std::os::windows::io::AsRawHandle;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use windows_sys::Win32::Foundation::{
        GetHandleInformation, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT,
    };

    /// One spawn at a time may hold the standard handles open for business.
    /// Clearing the inherit bit is process wide, so a second thread spawning a
    /// child that *does* want our stdout while we have it cleared would hand it
    /// a dead handle. The window is microseconds; the lock removes it for every
    /// spawn that goes through here.
    fn gate() -> &'static Mutex<()> {
        static GATE: OnceLock<Mutex<()>> = OnceLock::new();
        GATE.get_or_init(|| Mutex::new(()))
    }

    /// Our standard handles, made non-inheritable for as long as this lives.
    pub struct Sealed {
        _guard: MutexGuard<'static, ()>,
        restore: Vec<HANDLE>,
    }

    impl Sealed {
        pub fn new() -> Self {
            let guard = gate().lock().unwrap_or_else(|poison| poison.into_inner());
            let handles = [
                stdin().as_raw_handle() as HANDLE,
                stdout().as_raw_handle() as HANDLE,
                stderr().as_raw_handle() as HANDLE,
            ];
            let mut restore = Vec::new();
            for handle in handles {
                if handle.is_null() || !was_inheritable(handle) {
                    continue;
                }
                if unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, 0) } != 0 {
                    restore.push(handle);
                }
            }
            Self {
                _guard: guard,
                restore,
            }
        }
    }

    impl Drop for Sealed {
        fn drop(&mut self) {
            for handle in self.restore.drain(..) {
                unsafe {
                    SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT);
                }
            }
        }
    }

    /// Only touch handles that were inheritable to begin with, so we put back
    /// exactly what we found. A handle we cannot ask about is left alone.
    fn was_inheritable(handle: HANDLE) -> bool {
        let mut flags = 0u32;
        if unsafe { GetHandleInformation(handle, &mut flags) } == 0 {
            return false;
        }
        flags & HANDLE_FLAG_INHERIT != 0
    }
}

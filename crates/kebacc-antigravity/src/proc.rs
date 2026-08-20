//! How this crate starts other processes on Windows without putting a console
//! window on the user's screen.
//!
//! Two different flags, for two different jobs, and the reason there are two is
//! the thing that bit us: a process started with `DETACHED_PROCESS` has no
//! console at all, so when *it* starts a console program - `git`, `icacls` -
//! Windows allocates a fresh console for that child and draws a window for it.
//! The detached refresh the status line spawns runs `git status`, and the status
//! line is drawn constantly, so that is a window flashing up over whatever the
//! user is doing, several times a minute.
//!
//! So: `detach` for the background copies of ourselves, `hidden` for every
//! external command, and `hidden` is not optional even inside a process that is
//! already detached.

use std::process::Command;

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

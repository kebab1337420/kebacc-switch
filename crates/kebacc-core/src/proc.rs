use std::process::{Child, Command};

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

    fn gate() -> &'static Mutex<()> {
        static GATE: OnceLock<Mutex<()>> = OnceLock::new();
        GATE.get_or_init(|| Mutex::new(()))
    }

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

    fn was_inheritable(handle: HANDLE) -> bool {
        let mut flags = 0u32;
        if unsafe { GetHandleInformation(handle, &mut flags) } == 0 {
            return false;
        }
        flags & HANDLE_FLAG_INHERIT != 0
    }
}

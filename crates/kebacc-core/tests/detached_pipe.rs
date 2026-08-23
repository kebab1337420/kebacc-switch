#![cfg(windows)]

use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::ReadFile;
use windows_sys::Win32::System::Console::{GetStdHandle, SetStdHandle, STD_OUTPUT_HANDLE};
use windows_sys::Win32::System::Pipes::CreatePipe;

#[test]
fn a_detached_child_does_not_hold_our_stdout_open() {
    let (read, write) = pipe();
    let saved = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    assert!(unsafe { SetStdHandle(STD_OUTPUT_HANDLE, write) } != 0);

    let mut command = Command::new("cmd");
    command
        .args(["/c", "ping -n 30 127.0.0.1"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    kebacc_core::proc::detach(&mut command);
    let child = kebacc_core::proc::spawn_detached(&mut command);

    unsafe { SetStdHandle(STD_OUTPUT_HANDLE, saved) };
    assert!(unsafe { CloseHandle(write) } != 0);

    let mut child = child.expect("the child starts");
    let (sender, receiver) = mpsc::channel();
    let reader = read as usize;
    std::thread::spawn(move || {
        let mut byte = [0u8; 1];
        let mut got = 0u32;
        let ok = unsafe {
            ReadFile(
                reader as HANDLE,
                byte.as_mut_ptr(),
                1,
                &mut got,
                std::ptr::null_mut(),
            )
        };
        let _ = sender.send((ok, got));
    });

    let answer = receiver.recv_timeout(Duration::from_secs(5));
    let _ = child.kill();
    let _ = child.wait();
    unsafe { CloseHandle(read) };

    match answer {
        Ok((_, got)) => assert_eq!(got, 0, "something was written to the pipe"),
        Err(_) => panic!("the detached child is still holding our stdout open"),
    }
}

fn pipe() -> (HANDLE, HANDLE) {
    let mut read: HANDLE = INVALID_HANDLE_VALUE;
    let mut write: HANDLE = INVALID_HANDLE_VALUE;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    assert!(unsafe { CreatePipe(&mut read, &mut write, &attributes, 0) } != 0);
    (read, write)
}

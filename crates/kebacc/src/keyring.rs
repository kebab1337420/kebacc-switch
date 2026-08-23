const SERVICE: &str = "gemini";
const ACCOUNT: &str = "antigravity";

const GO_KEYRING_BASE64: &str = "go-keyring-base64:";

fn decode(payload: &str) -> Option<String> {
    let payload = payload.trim();
    if payload.is_empty() {
        return None;
    }
    let Some(encoded) = payload.strip_prefix(GO_KEYRING_BASE64) else {
        return Some(payload.to_string());
    };
    use base64::Engine;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .ok()?;
    let text = String::from_utf8(decoded).ok()?;
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(windows)]
mod platform {
    use super::{ACCOUNT, SERVICE};
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{FILETIME, TRUE};
    use windows_sys::Win32::Security::Credentials::{
        CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    fn target() -> Vec<u16> {
        wide(&format!("{SERVICE}:{ACCOUNT}"))
    }

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub fn read() -> Option<String> {
        let target = target();
        let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
        let ok = unsafe {
            CredReadW(
                target.as_ptr(),
                CRED_TYPE_GENERIC,
                0,
                &mut credential as *mut _,
            )
        };
        if ok != TRUE || credential.is_null() {
            return None;
        }
        let payload = unsafe {
            let blob = std::slice::from_raw_parts(
                (*credential).CredentialBlob,
                (*credential).CredentialBlobSize as usize,
            );
            let text = String::from_utf8(blob.to_vec()).ok();
            CredFree(credential as *const _);
            text
        };
        super::decode(&payload?)
    }

    pub fn write(payload: &str) -> Result<(), String> {
        let mut target = target();
        let mut account = wide(ACCOUNT);
        let mut blob = payload.as_bytes().to_vec();
        let mut credential = CREDENTIALW {
            Flags: 0,
            Type: CRED_TYPE_GENERIC,
            TargetName: target.as_mut_ptr(),
            Comment: std::ptr::null_mut(),
            LastWritten: FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            },
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_mut_ptr(),
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: std::ptr::null_mut(),
            UserName: account.as_mut_ptr(),
        };
        let ok = unsafe { CredWriteW(&mut credential as *const _, 0) };
        if ok == TRUE {
            Ok(())
        } else {
            Err(format!(
                "CredWriteW failed: {}",
                std::io::Error::last_os_error()
            ))
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{ACCOUNT, SERVICE};

    pub fn read() -> Option<String> {
        let out = std::process::Command::new("security")
            .args(["find-generic-password", "-s", SERVICE, "-a", ACCOUNT, "-w"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        super::decode(&String::from_utf8_lossy(&out.stdout))
    }

    pub fn write(payload: &str) -> Result<(), String> {
        let status = std::process::Command::new("security")
            .args([
                "add-generic-password",
                "-s",
                SERVICE,
                "-a",
                ACCOUNT,
                "-w",
                payload,
                "-A",
                "-U",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|problem| format!("security could not be run: {problem}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("security refused to store the login".into())
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use super::{ACCOUNT, SERVICE};
    use std::io::Write;

    pub fn read() -> Option<String> {
        let out = std::process::Command::new("secret-tool")
            .args(["lookup", "service", SERVICE, "username", ACCOUNT])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        super::decode(&String::from_utf8_lossy(&out.stdout))
    }

    pub fn write(payload: &str) -> Result<(), String> {
        let mut child = std::process::Command::new("secret-tool")
            .args([
                "store",
                "--label=gemini",
                "service",
                SERVICE,
                "username",
                ACCOUNT,
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|problem| format!("secret-tool could not be run: {problem}"))?;
        child
            .stdin
            .as_mut()
            .ok_or("secret-tool took no input")?
            .write_all(payload.as_bytes())
            .map_err(|problem| format!("secret-tool would not take the login: {problem}"))?;
        let status = child
            .wait()
            .map_err(|problem| format!("secret-tool did not finish: {problem}"))?;
        if status.success() {
            Ok(())
        } else {
            Err("secret-tool refused to store the login".into())
        }
    }
}

const OFF: &str = "KEBACC_SWITCH_NO_KEYRING";

fn allowed() -> bool {
    std::env::var_os(OFF).is_none()
}

pub fn read() -> Option<String> {
    allowed().then(platform::read)?
}

pub fn write(payload: &str) -> Result<(), String> {
    if !allowed() {
        return Ok(());
    }
    platform::write(payload)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_plain_payload_is_left_alone() {
        assert_eq!(
            super::decode("  {\"token\":{}}  "),
            Some("{\"token\":{}}".into())
        );
    }

    #[test]
    fn a_go_keyring_payload_is_decoded() {
        assert_eq!(
            super::decode("go-keyring-base64:eyJ0b2tlbiI6e319"),
            Some("{\"token\":{}}".into())
        );
    }

    #[test]
    fn an_empty_entry_is_no_entry() {
        assert_eq!(super::decode("   "), None);
        assert_eq!(super::decode("go-keyring-base64:"), None);
    }
}

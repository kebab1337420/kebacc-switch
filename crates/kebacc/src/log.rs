//! What happened to a login, written down. Always on, because the failure this
//! exists for — a switch that lands on a login prompt — is over by the time
//! anyone thinks to turn a flag on and try again.
//!
//! Nothing secret goes in. A token is written as the first ten hex characters
//! of its SHA-256, which is enough to tell two tokens apart across a session
//! and worth nothing to whoever reads the file.

use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::PathBuf;

/// Where the file is rotated. One turn of the log is kept beside it.
const MAX_BYTES: u64 = 512 * 1024;

pub fn path() -> PathBuf {
    crate::provider::state_dir().join("kebacc.log")
}

fn rolled() -> PathBuf {
    crate::provider::state_dir().join("kebacc.log.1")
}

fn off() -> bool {
    std::env::var("KEBACC_SWITCH_LOG").is_ok_and(|flag| {
        matches!(
            flag.trim().to_ascii_lowercase().as_str(),
            "off" | "0" | "false" | "no"
        )
    })
}

pub fn line(text: &str) {
    if off() {
        return;
    }
    let file = path();
    rotate(&file);
    let Ok(mut handle) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
    else {
        return;
    };
    let _ = writeln!(
        handle,
        "{} [{}] {text}",
        crate::usage::now_iso(),
        std::process::id()
    );
    crate::provider::protect_new_file(&file);
}

fn rotate(file: &PathBuf) {
    let Ok(meta) = std::fs::metadata(file) else {
        return;
    };
    if meta.len() < MAX_BYTES {
        return;
    }
    let _ = std::fs::remove_file(rolled());
    let _ = std::fs::rename(file, rolled());
}

/// A token, said out loud without saying the token.
pub fn fingerprint(secret: Option<&str>) -> String {
    let Some(secret) = secret.filter(|s| !s.is_empty()) else {
        return "none".into();
    };
    Sha256::digest(secret.as_bytes())
        .iter()
        .take(5)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// An `expiresAt` in milliseconds, as a time a human can compare to now.
pub fn moment(ms: Option<i64>) -> String {
    let Some(ms) = ms else {
        return "unknown".into();
    };
    match chrono::DateTime::from_timestamp_millis(ms) {
        Some(at) => at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        None => "unreadable".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fingerprint_is_short_and_not_the_secret() {
        let print = fingerprint(Some("sk-ant-oat01-secret"));
        assert_eq!(print.len(), 10);
        assert!(!print.contains("secret"));
        assert_eq!(fingerprint(None), "none");
        assert_eq!(fingerprint(Some("")), "none");
    }

    #[test]
    fn two_tokens_do_not_share_a_fingerprint() {
        assert_ne!(fingerprint(Some("one")), fingerprint(Some("two")));
    }
}

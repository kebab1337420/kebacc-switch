use crate::jsonio;
use crate::lock;
use crate::provider::Provider;
use crate::seal;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

// One pool, so one memo each rather than a map keyed by which one.
fn creds_memo() -> &'static Mutex<Option<Option<String>>> {
    static MEMO: OnceLock<Mutex<Option<Option<String>>>> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(None))
}

fn identity_memo() -> &'static Mutex<Option<Option<Value>>> {
    static MEMO: OnceLock<Mutex<Option<Option<Value>>>> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(None))
}

pub fn forget(_provider: &Provider) {
    if let Ok(mut memo) = creds_memo().lock() {
        *memo = None;
    }
    if let Ok(mut memo) = identity_memo().lock() {
        *memo = None;
    }
}

pub fn creds_raw(provider: &Provider) -> Option<String> {
    if let Ok(memo) = creds_memo().lock() {
        if let Some(hit) = memo.as_ref() {
            return hit.clone();
        }
    }
    let fresh = read_creds_raw(provider);
    if let Ok(mut memo) = creds_memo().lock() {
        *memo = Some(fresh.clone());
    }
    fresh
}

fn read_creds_raw(provider: &Provider) -> Option<String> {
    // Codex keeps its login in a file on every platform, so there is no
    // Keychain path here the way the Claude half needs one.
    let file = provider.cred_file();
    let text = std::fs::read_to_string(&file).ok()?;
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

pub fn set_creds_raw(provider: &Provider, raw: &str) -> std::io::Result<()> {
    forget(provider);
    let file = provider.cred_file();
    jsonio::write_text(&file, raw)?;
    Ok(())
}

pub fn identity(provider: &Provider) -> Option<Value> {
    if let Ok(memo) = identity_memo().lock() {
        if let Some(hit) = memo.as_ref() {
            return hit.clone();
        }
    }
    let fresh = read_identity(provider);
    if let Ok(mut memo) = identity_memo().lock() {
        *memo = Some(fresh.clone());
    }
    fresh
}

fn read_identity(provider: &Provider) -> Option<Value> {
    let raw = creds_raw(provider)?;
    let creds: Value = serde_json::from_str(&raw).ok()?;
    codex_identity(&creds)
}

pub fn codex_identity(creds: &Value) -> Option<Value> {
    let tokens = creds.get("tokens").filter(|v| !v.is_null())?;
    let mut email = None;
    let mut uuid = jsonio::str_of(tokens, "account_id");
    if let Some(claims) = jsonio::str_of(tokens, "id_token").and_then(|t| jsonio::jwt_payload(&t)) {
        email = jsonio::str_of(&claims, "email");
        if uuid.is_none() {
            uuid = claims
                .get("https://api.openai.com/auth")
                .and_then(|auth| jsonio::str_of(auth, "chatgpt_account_id"));
        }
    }
    if email.is_none() && uuid.is_none() {
        return None;
    }
    Some(json!({ "emailAddress": email, "accountUuid": uuid }))
}

pub fn backup_creds(provider: &Provider) {
    let Some(raw) = creds_raw(provider) else {
        return;
    };
    let dir = provider.backup_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    crate::provider::protect_dir(&dir);
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let (name, body) = match seal::protect(&raw) {
        Some(sealed) => (format!("creds-{stamp}.ccx"), sealed),
        None => (format!("creds-{stamp}.json"), raw),
    };
    let file = dir.join(name);
    if std::fs::write(&file, body).is_err() {
        return;
    }
    crate::provider::protect_file(&file);
    for old in backup_files(provider).into_iter().skip(3) {
        let _ = std::fs::remove_file(old);
    }
}

pub fn backup_files(provider: &Provider) -> Vec<PathBuf> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> =
        match std::fs::read_dir(provider.backup_dir()) {
            Ok(dir) => dir
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name.starts_with("creds-") || name.starts_with("backup-")
                })
                .filter_map(|e| {
                    let at = e.metadata().and_then(|m| m.modified()).ok()?;
                    Some((at, e.path()))
                })
                .collect(),
            Err(_) => return Vec::new(),
        };
    files.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    files.into_iter().map(|(_, p)| p).collect()
}

pub struct Backup {
    pub raw: String,
    pub at: chrono::DateTime<chrono::Local>,
}

pub fn newest_backup(provider: &Provider) -> Option<Backup> {
    let file = backup_files(provider).into_iter().next()?;
    let text = std::fs::read_to_string(&file).ok()?.trim().to_string();
    let raw = if text.starts_with('{') {
        text
    } else {
        seal::unprotect(&text)?
    };
    if !raw.starts_with('{') {
        return None;
    }
    let at = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .map(chrono::DateTime::<chrono::Local>::from)
        .unwrap_or_else(|_| chrono::Local::now());
    Some(Backup { raw, at })
}

pub fn activate(provider: &Provider, entry: &crate::pool::Entry) -> Result<(), String> {
    let Some(creds) = entry.creds.as_deref() else {
        return Err(format!(
            "The credentials for {} could not be read back.",
            entry.email
        ));
    };
    lock::locked(lock::CRED_SWAP, || {
        backup_creds(provider);
        // The identity travels inside auth.json itself, so writing the
        // credentials back is the whole swap: there is no second file to keep
        // in step with it.
        set_creds_raw(provider, creds)
            .map_err(|e| format!("Could not write the credentials: {e}"))?;
        Ok(())
    })?
}

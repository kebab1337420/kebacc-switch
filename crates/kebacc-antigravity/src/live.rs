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
    // The CLI's flat file first: it holds the same payload the IDE keeps in the
    // credential store, and reading a file costs nothing. The credential store
    // is the fallback for a machine where only the IDE has ever signed in.
    let file = provider.cred_file();
    if let Ok(text) = std::fs::read_to_string(&file) {
        let text = text.trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    crate::keyring::read()
}

/// Writes the login everywhere Antigravity reads one back.
///
/// The CLI reads a file and the IDE reads the operating system's credential
/// store, and the two carry byte-identical payloads, so a switch that touched
/// only one of them would leave the other signed in as somebody else. The
/// credential store is best effort: a machine with no CLI installed, or a
/// locked keyring, must not fail a switch the file already accepted.
pub fn set_creds_raw(provider: &Provider, raw: &str) -> std::io::Result<()> {
    forget(provider);
    let file = provider.cred_file();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    jsonio::write_text(&file, raw)?;
    if let Err(problem) = crate::keyring::write(raw) {
        crate::usage::debug(&format!("credential store not written: {problem}"));
    }
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
    antigravity_identity(&creds)
}

/// The `token` object inside an Antigravity payload, whichever shape it
/// arrived in.
///
/// The credential store and the CLI file both hold `{"token": {...},
/// "auth_method": "consumer"}`; a payload handed over by other tooling is
/// sometimes the inner object on its own. Reading both costs one `get`.
pub fn token_of(creds: &Value) -> Option<&Value> {
    match creds.get("token").filter(|v| !v.is_null()) {
        Some(token) => Some(token),
        None => creds.get("refresh_token").is_some().then_some(creds),
    }
}

pub fn refresh_token(creds: &Value) -> Option<String> {
    jsonio::str_of(token_of(creds)?, "refresh_token")
}

/// Who a payload belongs to.
///
/// Unlike Codex, Antigravity's payload carries no address of its own: the
/// refresh token is the whole of it, and an `id_token` is only there when the
/// account was saved by tooling that kept one. So the address is looked for in
/// three places, cheapest first — the `id_token` when there is one, then the
/// saved pool, whose snapshots already pair a refresh token with the address it
/// was saved under, and only then Google, which is a request and is therefore
/// the last resort rather than the first.
///
/// The refresh token doubles as the account's stable id: it survives every
/// access-token refresh, and it is what tells two saved logins apart.
pub fn antigravity_identity(creds: &Value) -> Option<Value> {
    let refresh = refresh_token(creds)?;
    let mut email = jsonio::str_of(token_of(creds)?, "id_token")
        .and_then(|t| jsonio::jwt_payload(&t))
        .and_then(|claims| jsonio::str_of(&claims, "email"));
    if email.is_none() {
        email = email_from_pool(&refresh);
    }
    if email.is_none() {
        email = crate::usage::email_from_google(creds);
    }
    Some(json!({
        "emailAddress": email,
        "accountUuid": crate::pool::short_hash(&refresh),
    }))
}

/// The address a saved snapshot was filed under, found by the refresh token it
/// holds. Every switch writes the pool before it writes the login, so the pool
/// knows the answer for every account it carries — which is every account this
/// tool ever signed in as.
fn email_from_pool(refresh: &str) -> Option<String> {
    let store = crate::provider::spec().store;
    let snapshots = crate::pool::plain_snapshots(&store)?;
    snapshots.iter().find_map(|(_, snapshot)| {
        let creds: Value = serde_json::from_str(&crate::pool::snapshot_creds(snapshot)?).ok()?;
        if refresh_token(&creds)? != refresh {
            return None;
        }
        crate::pool::identity_of(snapshot)
            .and_then(|account| jsonio::str_of(&account, "emailAddress"))
            .or_else(|| jsonio::str_of(snapshot, "email"))
    })
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
        // Writing the payload back is the whole swap. `set_creds_raw` puts it
        // in both places Antigravity reads a login from -- the CLI file and
        // the credential store -- so there is no third file to keep in step
        // with it.
        set_creds_raw(provider, creds)
            .map_err(|e| format!("Could not write the credentials: {e}"))?;
        Ok(())
    })?
}

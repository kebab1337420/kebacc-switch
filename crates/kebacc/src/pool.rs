use crate::jsonio;
use crate::provider::Provider;
use crate::seal;
use crate::term::Color;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const POOL_VERSION: u64 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    Trusted,
    Changed,
    Unknown,
    NoKey,
}

impl Trust {
    pub fn verdict(self) -> (&'static str, Color) {
        match self {
            Trust::Trusted => ("trusted", Color::Dim),
            Trust::Changed => ("CHANGED", Color::Red),
            Trust::NoKey => ("unverified", Color::Yellow),
            Trust::Unknown => ("unknown", Color::Yellow),
        }
    }
}

pub struct Entry {
    pub email: String,
    pub file: PathBuf,
    pub snapshot: Value,
    pub creds: Option<String>,
    pub identity: Option<Value>,
    pub cache: Option<Value>,
    pub protected: bool,
    pub trust: Trust,
}

impl Entry {
    pub fn file_name(&self) -> String {
        self.file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()
    }
}

pub struct Pool<'a> {
    provider: &'a Provider,
}

impl<'a> Pool<'a> {
    pub fn new(provider: &'a Provider) -> Self {
        Self { provider }
    }

    fn key_file(&self) -> PathBuf {
        self.provider.store.join(".pool.key")
    }

    fn manifest_file(&self) -> PathBuf {
        self.provider.store.join(".pool.json")
    }

    pub fn key(&self, create: bool) -> Option<Vec<u8>> {
        let path = self.key_file();
        if path.exists() {
            let wrapped = std::fs::read(&path).ok()?;
            if cfg!(windows) {
                return seal::unwrap_bytes(&wrapped);
            }
            let sealed = String::from_utf8(wrapped).ok()?;
            let plain = seal::unprotect(&sealed)?;
            return B64.decode(plain.as_bytes()).ok();
        }
        if !create {
            return None;
        }
        std::fs::create_dir_all(&self.provider.store).ok()?;
        crate::provider::protect_dir(&self.provider.store);
        let key = seal::random_bytes(32);
        if cfg!(windows) {
            std::fs::write(&path, seal::wrap_bytes(&key)?).ok()?;
        } else {
            std::fs::write(&path, seal::protect(&B64.encode(&key))?.as_bytes()).ok()?;
        }
        crate::provider::protect_file(&path);
        Some(key)
    }

    pub fn entries(&self) -> Vec<Entry> {
        let key = self.key(false);
        let mut files: Vec<PathBuf> = match std::fs::read_dir(&self.provider.store) {
            Ok(dir) => dir
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.extension().map(|e| e == "json").unwrap_or(false)
                        && !p
                            .file_name()
                            .map(|n| n.to_string_lossy().starts_with('.'))
                            .unwrap_or(true)
                })
                .collect(),
            Err(_) => return Vec::new(),
        };
        files.sort();

        files
            .into_iter()
            .filter_map(|file| {
                let snapshot = jsonio::read(&file)?;
                let name = file.file_name()?.to_string_lossy().to_string();
                Some(Entry {
                    email: jsonio::str_of(&snapshot, "email").unwrap_or_default(),
                    creds: snapshot_creds(&snapshot),
                    identity: identity_of(&snapshot),
                    cache: jsonio::obj(&snapshot, "usageCache"),
                    protected: snapshot.get("credentialsProtected").is_some(),
                    trust: self.verify(key.as_deref(), &name, &snapshot),
                    file,
                    snapshot,
                })
            })
            .collect()
    }

    pub fn manifest(&self) -> Option<Value> {
        jsonio::read(&self.manifest_file())
    }

    pub fn register(&self, file_name: &str, snapshot: &Value) -> bool {
        let Some(key) = self.key(true) else {
            return false;
        };
        let (uuid, email) = pool_identity(snapshot);
        let Some(uuid) = uuid else { return false };
        let cred_hash = cred_hash(snapshot_creds(snapshot).as_deref());
        let stamp = stamp(&key, file_name, &email, &uuid, Some(&cred_hash));

        let mut manifest = self
            .manifest()
            .unwrap_or_else(|| json!({ "version": POOL_VERSION, "accounts": Map::new() }));
        let map = jsonio::map_mut(&mut manifest);
        map.insert("version".into(), json!(POOL_VERSION));
        if !map.get("accounts").map(Value::is_object).unwrap_or(false) {
            map.insert("accounts".into(), Value::Object(Map::new()));
        }
        let accounts = map
            .get_mut("accounts")
            .and_then(Value::as_object_mut)
            .expect("just ensured accounts is an object");
        accounts.insert(
            file_name.to_string(),
            json!({
                "email": email,
                "accountUuid": uuid,
                "credHash": cred_hash,
                "stamp": stamp,
                "registered": crate::usage::now_iso(),
            }),
        );
        jsonio::write(&self.manifest_file(), &manifest).is_ok()
    }

    pub fn unregister(&self, file_name: &str) {
        let Some(mut manifest) = self.manifest() else {
            return;
        };
        let Some(accounts) = manifest.get_mut("accounts").and_then(Value::as_object_mut) else {
            return;
        };
        if accounts.remove(file_name).is_none() {
            return;
        }
        let _ = jsonio::write(&self.manifest_file(), &manifest);
    }

    fn verify(&self, key: Option<&[u8]>, file_name: &str, snapshot: &Value) -> Trust {
        let Some(key) = key else { return Trust::NoKey };
        let Some(manifest) = self.manifest() else {
            return Trust::Unknown;
        };
        let Some(entry) = manifest.get("accounts").and_then(|a| a.get(file_name)) else {
            return Trust::Unknown;
        };
        let recorded_email = jsonio::str_of(entry, "email").unwrap_or_default();
        let recorded_uuid = jsonio::str_of(entry, "accountUuid").unwrap_or_default();
        let recorded_stamp = jsonio::str_of(entry, "stamp").unwrap_or_default();
        let (uuid, email) = pool_identity(snapshot);
        let uuid = uuid.unwrap_or_default();
        if !email.eq_ignore_ascii_case(&recorded_email) {
            return Trust::Changed;
        }
        let cred_hash = cred_hash(snapshot_creds(snapshot).as_deref());

        let expected = stamp(
            key,
            file_name,
            &recorded_email,
            &recorded_uuid,
            Some(&cred_hash),
        );
        if same_stamp(&recorded_stamp, &expected) && uuid == recorded_uuid {
            return Trust::Trusted;
        }

        for legacy in [None, Some("none")] {
            let old = stamp(key, file_name, &recorded_email, &recorded_uuid, legacy);
            if !same_stamp(&recorded_stamp, &old) || uuid != recorded_uuid {
                continue;
            }
            self.register(file_name, snapshot);
            return Trust::Trusted;
        }
        Trust::Changed
    }
}

pub fn snapshot_creds(snapshot: &Value) -> Option<String> {
    if let Some(sealed) = jsonio::str_of(snapshot, "credentialsProtected") {
        return seal::unprotect(&sealed);
    }
    match snapshot.get("credentials") {
        Some(value) if !value.is_null() => serde_json::to_string(value).ok(),
        _ => None,
    }
}

pub fn identity_of(snapshot: &Value) -> Option<Value> {
    match snapshot.get("oauthAccountRaw") {
        Some(Value::String(text)) => serde_json::from_str(text).ok(),
        Some(Value::Null) | None => None,
        Some(other) => Some(other.clone()),
    }
}

fn pool_identity(snapshot: &Value) -> (Option<String>, String) {
    let mut uuid = None;
    let mut email = None;
    if let Some(account) = identity_of(snapshot) {
        uuid = jsonio::str_of(&account, "accountUuid");
        email = jsonio::str_of(&account, "emailAddress");
    }
    if email.is_none() {
        email = jsonio::str_of(snapshot, "email");
    }
    (uuid, email.unwrap_or_default())
}

fn sha256_hex(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// A short, stable name for an account, taken from a secret that must not be
/// stored or shown. Antigravity has no account id of its own; the refresh
/// token is the value that stays put, so the digest is what is filed.
pub fn short_hash(text: &str) -> String {
    sha256_hex(text).chars().take(12).collect()
}

fn cred_hash(creds_raw: Option<&str>) -> String {
    let Some(raw) = creds_raw else {
        return "none".into();
    };
    let Ok(creds) = serde_json::from_str::<Value>(raw) else {
        return "none".into();
    };
    if let Some(oauth) = creds.get("claudeAiOauth").filter(|v| !v.is_null()) {
        return sha256_hex(&format!(
            "{}|{}|{}",
            jsonio::str_of(oauth, "accessToken").unwrap_or_default(),
            jsonio::str_of(oauth, "refreshToken").unwrap_or_default(),
            jsonio::str_of(oauth, "expiresAt").unwrap_or_default(),
        ));
    }
    if let Some(tokens) = creds.get("tokens").filter(|v| !v.is_null()) {
        return sha256_hex(&format!(
            "{}|{}|{}",
            jsonio::str_of(tokens, "access_token").unwrap_or_default(),
            jsonio::str_of(tokens, "refresh_token").unwrap_or_default(),
            jsonio::str_of(tokens, "account_id").unwrap_or_default(),
        ));
    }
    if let Some(key) = jsonio::str_of(&creds, "OPENAI_API_KEY") {
        return sha256_hex(&key);
    }
    if let Some(token) = crate::live::token_of(&creds) {
        // The access token is left out: Antigravity refreshes it behind this
        // tool's back, and a hash that moved every hour would call every saved
        // login tampered with.
        return sha256_hex(&format!(
            "{}|{}",
            jsonio::str_of(token, "refresh_token").unwrap_or_default(),
            jsonio::str_of(token, "id_token").unwrap_or_default(),
        ));
    }
    "none".into()
}

fn stamp(key: &[u8], file_name: &str, email: &str, uuid: &str, cred_hash: Option<&str>) -> String {
    let payload = match cred_hash {
        Some(hash) => format!("{file_name}|{email}|{uuid}|{hash}"),
        None => format!("{file_name}|{email}|{uuid}"),
    };
    let mut mac = <Hmac<Sha256>>::new_from_slice(key).expect("HMAC takes a key of any length");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn new_snapshot(
    email: &str,
    creds_raw: &str,
    identity: Option<&Value>,
    usage_cache: Option<&Value>,
    saved_at: Option<&str>,
) -> (Value, bool) {
    let mut entry = Map::new();
    entry.insert("email".into(), json!(email));
    entry.insert(
        "savedAt".into(),
        json!(saved_at
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(crate::usage::now_iso)),
    );
    let sealed = seal::protect(creds_raw);
    let protected = sealed.is_some();
    match sealed {
        Some(text) => {
            entry.insert("credentialsProtected".into(), json!(text));
        }
        None => {
            let parsed: Value = serde_json::from_str(creds_raw).unwrap_or(Value::Null);
            entry.insert("credentials".into(), parsed);
        }
    }
    if let Some(identity) = identity {
        entry.insert("oauthAccountRaw".into(), identity.clone());
    }
    if let Some(cache) = usage_cache {
        entry.insert("usageCache".into(), cache.clone());
    }
    (Value::Object(entry), protected)
}

/// Put a fresh credentials document into a snapshot that already exists,
/// keeping everything else it holds, and stamp it again so the pool still
/// counts it as one this machine registered.
pub fn save_creds(provider: &Provider, file: &Path, raw: &str) -> bool {
    let previous = jsonio::read(file);
    let email = previous
        .as_ref()
        .and_then(|p| jsonio::str_of(p, "email"))
        .unwrap_or_default();
    if email.is_empty() {
        return false;
    }
    let identity = previous.as_ref().and_then(identity_of);
    let cache = previous.as_ref().and_then(|p| jsonio::obj(p, "usageCache"));
    let saved_at = previous.as_ref().and_then(|p| jsonio::str_of(p, "savedAt"));
    let (snapshot, _) = new_snapshot(
        &email,
        raw,
        identity.as_ref(),
        cache.as_ref(),
        saved_at.as_deref(),
    );
    if jsonio::write(file, &snapshot).is_err() {
        return false;
    }
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Pool::new(provider).register(&name, &snapshot);
    true
}

pub fn snapshot_files(store: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(store) {
        Ok(dir) => dir.filter_map(|e| e.ok()).map(|e| e.path()).collect(),
        Err(_) => Vec::new(),
    }
}

type Snapshots = Option<Vec<(PathBuf, Value)>>;

fn snapshot_memo() -> &'static Mutex<HashMap<PathBuf, Snapshots>> {
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, Snapshots>>> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn forget_snapshots() {
    if let Ok(mut memo) = snapshot_memo().lock() {
        memo.clear();
    }
}

pub fn plain_snapshots(store: &Path) -> Snapshots {
    if let Ok(memo) = snapshot_memo().lock() {
        if let Some(known) = memo.get(store) {
            return known.clone();
        }
    }
    let fresh = read_plain_snapshots(store);
    if let Ok(mut memo) = snapshot_memo().lock() {
        memo.insert(store.to_path_buf(), fresh.clone());
    }
    fresh
}

fn read_plain_snapshots(store: &Path) -> Snapshots {
    if !store.is_dir() {
        return None;
    }
    let mut files: Vec<PathBuf> = snapshot_files(store)
        .into_iter()
        .filter(|file| {
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            name.ends_with(".json") && !name.starts_with('.')
        })
        .collect();
    files.sort();
    Some(
        files
            .into_iter()
            .map(|file| {
                let snapshot = crate::jsonio::read(&file).unwrap_or(Value::Null);
                (file, snapshot)
            })
            .collect(),
    )
}

fn same_stamp(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ProviderId;

    fn temp_provider(name: &str) -> Provider {
        let store = std::env::temp_dir().join(format!(
            "kebacc-pool-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&store).expect("a store to write into");
        Provider {
            id: ProviderId::Claude,
            label: "Claude Code",
            cli: "claude",
            store,
            cred_candidates: Vec::new(),
            config_candidates: Vec::new(),
            cred_label: "test",
            uses_keychain: false,
            keychain_service: None,
        }
    }

    fn creds(access: &str) -> String {
        json!({ "claudeAiOauth": { "accessToken": access, "refreshToken": "r", "expiresAt": 1 } })
            .to_string()
    }

    #[test]
    fn a_new_pair_replaces_the_old_one_and_keeps_the_rest() {
        let provider = temp_provider("save");
        let (snapshot, _) = new_snapshot(
            "one@example.com",
            &creds("old"),
            Some(&json!({ "emailAddress": "one@example.com", "accountUuid": "u-1" })),
            Some(&json!({ "checkedAt": "2026-01-01T00:00:00Z" })),
            Some("2026-01-01T00:00:00Z"),
        );
        let file = provider.snapshot_path("one@example.com");
        jsonio::write(&file, &snapshot).expect("the snapshot is written");
        Pool::new(&provider).register(&file.file_name().unwrap().to_string_lossy(), &snapshot);

        assert!(save_creds(&provider, &file, &creds("new")));

        let entries = Pool::new(&provider).entries();
        let entry = entries.first().expect("the account is still there");
        assert_eq!(entry.email, "one@example.com");
        assert_eq!(entry.creds.as_deref(), Some(creds("new").as_str()));
        assert_eq!(
            jsonio::str_of(&entry.snapshot, "savedAt").as_deref(),
            Some("2026-01-01T00:00:00Z")
        );
        assert!(entry.cache.is_some());
        // The stamp is written again, so the pool still knows the account. A
        // machine with no secret backend, which is most CI Linux, holds no key
        // to stamp with and reads every snapshot as unverified. What has to
        // hold everywhere is that the rewrite never reads as CHANGED.
        let stamped = Pool::new(&provider).key(false).is_some();
        assert!(matches!(entry.trust, Trust::Trusted | Trust::NoKey));
        assert!(!stamped || matches!(entry.trust, Trust::Trusted));
        let _ = std::fs::remove_dir_all(&provider.store);
    }

    #[test]
    fn a_snapshot_that_is_not_there_is_not_invented() {
        let provider = temp_provider("missing");
        let file = provider.snapshot_path("nobody@example.com");
        assert!(!save_creds(&provider, &file, &creds("new")));
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(&provider.store);
    }
}

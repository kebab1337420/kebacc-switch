use crate::branch::Identity;
use crate::jsonio;
use crate::lock;
use crate::provider::Provider;
use crate::provider::ProviderId;
use crate::seal;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

type Memo<T> = OnceLock<Mutex<HashMap<ProviderId, Option<T>>>>;

fn creds_memo() -> &'static Mutex<HashMap<ProviderId, Option<String>>> {
    static MEMO: Memo<String> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(HashMap::new()))
}

fn identity_memo() -> &'static Mutex<HashMap<ProviderId, Option<Value>>> {
    static MEMO: Memo<Value> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn forget(provider: &Provider) {
    if let Ok(mut memo) = creds_memo().lock() {
        memo.remove(&provider.id);
    }
    if let Ok(mut memo) = identity_memo().lock() {
        memo.remove(&provider.id);
    }
}

pub fn creds_raw(provider: &Provider) -> Option<String> {
    if let Ok(memo) = creds_memo().lock() {
        if let Some(hit) = memo.get(&provider.id) {
            return hit.clone();
        }
    }
    let fresh = read_creds_raw(provider);
    if let Ok(mut memo) = creds_memo().lock() {
        memo.insert(provider.id, fresh.clone());
    }
    fresh
}

fn read_creds_raw(provider: &Provider) -> Option<String> {
    let file = provider.cred_file();
    if provider.uses_keychain && !file.exists() {
        let service = provider.keychain_service?;
        let mut security = std::process::Command::new("security");
        let out = crate::proc::hidden(&mut security)
            .args(["find-generic-password", "-s", service, "-w"])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return (!text.is_empty()).then_some(text);
    }
    if let Ok(text) = std::fs::read_to_string(&file) {
        let text = text.trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    if provider.id.branch().uses_keyring {
        return crate::keyring::read();
    }
    None
}

pub fn set_creds_raw(provider: &Provider, raw: &str) -> std::io::Result<()> {
    forget(provider);
    let file = provider.cred_file();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    jsonio::write_text(&file, raw)?;
    if provider.uses_keychain {
        if let Some(service) = provider.keychain_service {
            let user = std::env::var("USER").unwrap_or_default();
            seal::secret_via_stdin(
                "security",
                &[
                    "add-generic-password",
                    "-U",
                    "-s",
                    service,
                    "-a",
                    &user,
                    "-w",
                ],
                &format!(
                    "{raw}
{raw}
"
                ),
            );
        }
    }
    if provider.id.branch().uses_keyring {
        if let Err(problem) = crate::keyring::write(raw) {
            crate::usage::debug(&format!("credential store not written: {problem}"));
        }
    }
    Ok(())
}

pub fn identity(provider: &Provider) -> Option<Value> {
    if let Ok(memo) = identity_memo().lock() {
        if let Some(hit) = memo.get(&provider.id) {
            return hit.clone();
        }
    }
    let fresh = read_identity(provider);
    if let Ok(mut memo) = identity_memo().lock() {
        memo.insert(provider.id, fresh.clone());
    }
    fresh
}

fn read_identity(provider: &Provider) -> Option<Value> {
    match provider.id.branch().identity {
        Identity::ConfigMember(member) => {
            let config = jsonio::read(&provider.config_file())?;
            jsonio::obj(&config, member)
        }
        Identity::Codex => {
            let raw = creds_raw(provider)?;
            let creds: Value = serde_json::from_str(&raw).ok()?;
            codex_identity(&creds)
        }
        Identity::Antigravity => {
            let raw = creds_raw(provider)?;
            let creds: Value = serde_json::from_str(&raw).ok()?;
            antigravity_identity(&creds)
        }
        Identity::Search => {
            let raw = creds_raw(provider)?;
            let creds: Value = serde_json::from_str(&raw).ok()?;
            searched_identity(&creds)
        }
    }
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

pub fn token_of(creds: &Value) -> Option<&Value> {
    match creds.get("token").filter(|v| !v.is_null()) {
        Some(token) => Some(token),
        None => creds.get("refresh_token").is_some().then_some(creds),
    }
}

pub fn refresh_token(creds: &Value) -> Option<String> {
    jsonio::str_of(token_of(creds)?, "refresh_token")
}

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

fn email_from_pool(refresh: &str) -> Option<String> {
    let store = crate::provider::spec(ProviderId::Antigravity).store;
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

pub fn searched_identity(creds: &Value) -> Option<Value> {
    let email = crate::usage::deep_str(creds, "email")
        .or_else(|| crate::usage::deep_str(creds, "user_email"))
        .or_else(|| {
            crate::usage::deep_str(creds, "id_token")
                .and_then(|token| jsonio::jwt_payload(&token))
                .and_then(|claims| jsonio::str_of(&claims, "email"))
        });
    let uuid = crate::usage::deep_str(creds, "user_id")
        .or_else(|| crate::usage::deep_str(creds, "account_id"))
        .or_else(|| {
            crate::usage::deep_str(creds, "access_token")
                .map(|token| crate::pool::short_hash(&token))
        });
    if email.is_none() && uuid.is_none() {
        return None;
    }
    Some(json!({ "emailAddress": email, "accountUuid": uuid }))
}

pub fn set_identity(provider: &Provider, identity: &Value) {
    let Identity::ConfigMember(member) = provider.id.branch().identity else {
        return;
    };
    forget(provider);
    let path = provider.config_file();
    if !path.exists() {
        let _ = jsonio::write(&path, &json!({ member: identity }));
        return;
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let block = serde_json::to_string(identity).unwrap_or_else(|_| "{}".into());

    let updated = match find_member(&text, member) {
        Some((start, end)) => {
            let mut out = String::with_capacity(text.len() + block.len());
            out.push_str(&text[..start]);
            out.push_str(&block);
            out.push_str(&text[end..]);
            out
        }
        None => {
            let Some(open) = text.find('{') else { return };
            let rest = text[open + 1..].trim_start();
            let comma = if rest.starts_with('}') { "" } else { "," };
            let mut out = String::with_capacity(text.len() + block.len() + 20);
            out.push_str(&text[..=open]);
            out.push('"');
            out.push_str(member);
            out.push_str("\":");
            out.push_str(&block);
            out.push_str(comma);
            out.push_str(&text[open + 1..]);
            out
        }
    };
    let _ = jsonio::write_text(&path, &updated);
}

fn find_member(text: &str, name: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut i = skip_space(bytes, 0);
    if i >= bytes.len() || bytes[i] != b'{' {
        return None;
    }
    i += 1;
    loop {
        i = skip_space(bytes, i);
        if i >= bytes.len() || bytes[i] == b'}' {
            return None;
        }
        if bytes[i] != b'"' {
            return None;
        }
        let (key, after) = read_string(bytes, i)?;
        i = skip_space(bytes, after);
        if i >= bytes.len() || bytes[i] != b':' {
            return None;
        }
        i = skip_space(bytes, i + 1);
        let start = i;
        i = skip_value(bytes, i)?;
        if key == name {
            return Some((start, i));
        }
        i = skip_space(bytes, i);
        match bytes.get(i) {
            Some(b',') => i += 1,
            _ => return None,
        }
    }
}

fn skip_space(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn read_string(bytes: &[u8], i: usize) -> Option<(String, usize)> {
    let mut end = i + 1;
    while end < bytes.len() && bytes[end] != b'"' {
        if bytes[end] == b'\\' {
            end += 1;
        }
        end += 1;
    }
    if end >= bytes.len() {
        return None;
    }
    let text = std::str::from_utf8(&bytes[i + 1..end]).ok()?.to_string();
    Some((text, end + 1))
}

fn skip_value(bytes: &[u8], i: usize) -> Option<usize> {
    match *bytes.get(i)? {
        b'"' => read_string(bytes, i).map(|(_, end)| end),
        open @ (b'{' | b'[') => {
            let close = if open == b'{' { b'}' } else { b']' };
            let mut depth = 0usize;
            let mut at = i;
            while at < bytes.len() {
                match bytes[at] {
                    b'"' => at = read_string(bytes, at)?.1 - 1,
                    b if b == open => depth += 1,
                    b if b == close => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(at + 1);
                        }
                    }
                    _ => {}
                }
                at += 1;
            }
            None
        }
        _ => {
            let mut at = i;
            while at < bytes.len() && !matches!(bytes[at], b',' | b'}' | b']') {
                at += 1;
            }
            (at > i).then_some(at)
        }
    }
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
    if jsonio::write_private(&file, body.as_bytes()).is_err() {
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

pub struct Activation {
    pub renewed: bool,
    pub warning: Option<String>,
}

fn active_file(provider: &Provider) -> PathBuf {
    crate::provider::session_dir().join(format!("active-{}.json", provider.id.as_str()))
}

fn remember_active(provider: &Provider, email: &str, raw: &str) {
    let record = json!({
        "email": email,
        "accessFingerprint": crate::log::fingerprint(crate::oauth::access_token(raw).as_deref()),
        "at": crate::usage::now_iso(),
    });
    let _ = jsonio::write(&active_file(provider), &record);
}

fn active_email(provider: &Provider) -> Option<String> {
    jsonio::str_of(&jsonio::read(&active_file(provider))?, "email")
}

pub fn capture(provider: &Provider, pool: &[crate::pool::Entry]) {
    let Some(raw) = creds_raw(provider) else {
        return;
    };
    let print = crate::log::fingerprint(crate::oauth::access_token(&raw).as_deref());
    let identity_email = identity(provider)
        .as_ref()
        .and_then(|id| jsonio::str_of(id, "emailAddress"));
    let remembered = active_email(provider);
    if let (Some(live), Some(last)) = (identity_email.as_deref(), remembered.as_deref()) {
        if !live.eq_ignore_ascii_case(last) {
            crate::log::line(&format!(
                "capture: the CLI names {live} but the last switch was to {last}; going with {live}"
            ));
        }
    }
    let Some(owner) = identity_email.or(remembered) else {
        crate::log::line(&format!(
            "capture: no owner for the live pair {print}, leaving the pool alone"
        ));
        return;
    };
    let Some(entry) = pool.iter().find(|e| e.email.eq_ignore_ascii_case(&owner)) else {
        crate::log::line(&format!(
            "capture: {owner} is not in the pool, nothing to save"
        ));
        return;
    };
    if entry.creds.as_deref() == Some(raw.as_str()) {
        return;
    }
    if entry.trust == crate::pool::Trust::Changed {
        crate::log::line(&format!(
            "capture: {owner} is not the account this machine registered, refusing to write over it"
        ));
        return;
    }
    let saved = crate::pool::save_creds(provider, &entry.file, &raw);
    crate::log::line(&format!(
        "capture: {owner} pair {print} expiring {} {}",
        crate::log::moment(crate::oauth::expires_at(&raw)),
        if saved { "saved" } else { "COULD NOT BE SAVED" }
    ));
}

fn announce_switch(provider: &Provider, from: Option<&str>, to: &str) {
    let asked = match std::env::var("KEBACC_SWITCH_ON_SWITCH") {
        Ok(text) if !text.trim().is_empty() => Some(text),
        Ok(_) => None,
        Err(_) => crate::pool::Pool::new(provider).on_switch(),
    };
    let Some(asked) = asked else {
        return;
    };
    let mut command = if cfg!(windows) {
        let mut command = std::process::Command::new("cmd");
        command.args(["/c", &asked]);
        command
    } else {
        let mut command = std::process::Command::new("sh");
        command.args(["-c", &asked]);
        command
    };
    command
        .env("KEBACC_POOL", provider.label)
        .env("KEBACC_CLI", provider.cli)
        .env("KEBACC_TO", to)
        .env("KEBACC_FROM", from.unwrap_or_default())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::proc::detach(&mut command);
    let started = crate::proc::spawn_detached(&mut command).is_ok();
    crate::log::line(&format!(
        "switch: on-switch command {}",
        if started {
            "started"
        } else {
            "COULD NOT START"
        }
    ));
}

pub fn activate(provider: &Provider, entry: &crate::pool::Entry) -> Result<Activation, String> {
    let Some(creds) = entry.creds.as_deref() else {
        crate::log::line(&format!(
            "switch: {} could not be read back out of the pool",
            entry.email
        ));
        return Err(format!(
            "The credentials for {} could not be read back.",
            entry.email
        ));
    };
    crate::log::line(&format!(
        "switch: to {}, saved pair {} expiring {}",
        entry.email,
        crate::log::fingerprint(crate::oauth::access_token(creds).as_deref()),
        crate::log::moment(crate::oauth::expires_at(creds)),
    ));
    lock::locked(lock::CRED_SWAP, || {
        backup_creds(provider);
        capture(provider, &crate::pool::Pool::new(provider).entries());

        let mut warning = None;
        let mut renewed = false;
        let renewal = if provider.id.branch().renew {
            crate::oauth::renew_if_stale(creds)
        } else {
            crate::oauth::Renewal::Fresh
        };
        let creds = match renewal {
            crate::oauth::Renewal::Fresh => creds.to_string(),
            crate::oauth::Renewal::Renewed(fresh) => {
                renewed = true;
                let saved = crate::pool::save_creds(provider, &entry.file, &fresh);
                crate::log::line(&format!(
                    "switch: renewed {} to pair {} expiring {} ({})",
                    entry.email,
                    crate::log::fingerprint(crate::oauth::access_token(&fresh).as_deref()),
                    crate::log::moment(crate::oauth::expires_at(&fresh)),
                    if saved {
                        "written back to the pool"
                    } else {
                        "NOT written back to the pool"
                    }
                ));
                fresh
            }
            crate::oauth::Renewal::Failed(problem) => {
                crate::log::line(&format!(
                    "switch: could not renew {}: {problem}",
                    entry.email
                ));
                warning = Some(format!(
                    "The saved pair for {} is out of date and could not be renewed ({problem}). If the CLI asks for a login, run /login and save the account again.",
                    entry.email
                ));
                creds.to_string()
            }
        };

        set_creds_raw(provider, &creds)
            .map_err(|e| format!("Could not write the credentials: {e}"))?;
        if let Some(identity) = entry.identity.as_ref() {
            set_identity(provider, identity);
        }
        let leaving = active_email(provider);
        remember_active(provider, &entry.email, &creds);
        crate::log::line(&format!("switch: {} is now the live login", entry.email));
        let _ = std::fs::write(
            crate::provider::session_dir().join("switch.last"),
            format!(
                "{} {} {}",
                crate::usage::now_iso(),
                provider.label,
                entry.email
            ),
        );
        announce_switch(provider, leaving.as_deref(), &entry.email);
        Ok(Activation { renewed, warning })
    })?
}

#[cfg(test)]
mod tests {
    use super::find_member;

    #[test]
    fn finds_a_member_of_the_root_object() {
        let text = r#"{"a":1,"oauthAccount":{"emailAddress":"one@example.com"},"b":2}"#;
        let (start, end) = find_member(text, "oauthAccount").unwrap();
        assert_eq!(&text[start..end], r#"{"emailAddress":"one@example.com"}"#);
    }

    #[test]
    fn ignores_the_same_name_deeper_in_the_file() {
        let text = r#"{"projects":{"x":{"history":[{"display":"\"oauthAccount\": {\"emailAddress\": \"nobody\"}"}]}},"oauthAccount":{"emailAddress":"real@example.com"}}"#;
        let (start, end) = find_member(text, "oauthAccount").unwrap();
        assert_eq!(&text[start..end], r#"{"emailAddress":"real@example.com"}"#);
    }

    #[test]
    fn a_member_that_is_not_there_is_not_invented() {
        let text = r#"{"projects":{"oauthAccount":{"emailAddress":"nested@example.com"}}}"#;
        assert!(find_member(text, "oauthAccount").is_none());
    }

    #[test]
    fn finds_the_account_in_the_real_config() {
        let path = crate::provider::home().join(".claude.json");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let Some(expected) = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|config| crate::jsonio::obj(&config, "oauthAccount"))
        else {
            return;
        };
        let (start, end) = find_member(&text, "oauthAccount").expect("the member is in the file");
        let spliced: serde_json::Value =
            serde_json::from_str(&text[start..end]).expect("the range is one JSON value");
        assert_eq!(spliced, expected);
    }

    #[test]
    fn replaces_a_member_of_any_shape() {
        let text = r#"{"oauthAccount":null,"b":2}"#;
        let (start, end) = find_member(text, "oauthAccount").unwrap();
        assert_eq!(&text[start..end], "null");
    }
}

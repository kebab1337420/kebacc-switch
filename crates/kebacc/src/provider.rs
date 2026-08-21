use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const PROTECTED_MARK: &str = ".protected";
const AGY_SESSION_DIR: [&str; 2] = [".gemini", "antigravity-cli"];
const AGY_TOKEN_FILE: &str = "antigravity-oauth-token";

pub const PROVIDER_IDS: [&str; 3] = ["claude", "codex", "antigravity"];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ProviderId {
    Claude,
    Codex,
    Antigravity,
}

impl ProviderId {
    pub const ALL: [ProviderId; 3] = [
        ProviderId::Claude,
        ProviderId::Codex,
        ProviderId::Antigravity,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ProviderId::Claude => "claude",
            ProviderId::Codex => "codex",
            ProviderId::Antigravity => "antigravity",
        }
    }

    /// Keychain / libsecret account the AES wrapping key is stored under.
    /// Saved logins on existing machines only open if this stays this string.
    pub fn seal_account(self) -> &'static str {
        match self {
            ProviderId::Antigravity => "kebacc-antigravity",
            ProviderId::Claude | ProviderId::Codex => "kebacc-switch",
        }
    }
}

/// What `-Provider` asked for: one pool, or every pool this binary carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wanted {
    All,
    One(ProviderId),
}

impl Wanted {
    pub fn ids(self) -> Vec<ProviderId> {
        match self {
            Wanted::All => ProviderId::ALL.to_vec(),
            Wanted::One(id) => vec![id],
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Wanted::All => "all",
            Wanted::One(id) => id.as_str(),
        }
    }
}

pub struct Provider {
    pub id: ProviderId,
    pub label: &'static str,
    pub cli: &'static str,
    pub store: PathBuf,
    pub cred_candidates: Vec<PathBuf>,
    pub config_candidates: Vec<PathBuf>,
    pub cred_label: &'static str,
    pub uses_keychain: bool,
    pub keychain_service: Option<&'static str>,
}

pub fn home() -> PathBuf {
    static HOME: OnceLock<PathBuf> = OnceLock::new();
    HOME.get_or_init(|| match dirs::home_dir() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => {
            eprintln!(
                "! No home directory: refusing to read or write credentials relative to the current directory."
            );
            std::process::exit(1);
        }
    })
    .clone()
}

pub fn state_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = match std::env::var_os("KEBACC_SWITCH_STATE_DIR") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => home().join(".kebacc-switch"),
        };
        if std::fs::create_dir_all(&dir).is_ok() {
            protect_dir(&dir);
        }
        dir
    })
    .clone()
}

pub fn claude_config_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home().join(".claude"),
    })
    .clone()
}

pub fn is_all(id: &str) -> bool {
    matches!(
        id.trim().to_ascii_lowercase().as_str(),
        "all" | "every" | "*"
    )
}

pub fn resolve(id: &str) -> Result<Wanted, String> {
    let key = id.trim().to_ascii_lowercase();
    if key.is_empty() || is_all(&key) {
        return Ok(Wanted::All);
    }
    match key.as_str() {
        "claude" | "claude-code" | "claudecode" | "cc" | "anthropic" => {
            Ok(Wanted::One(ProviderId::Claude))
        }
        "codex" | "openai" | "chatgpt" | "gpt" => Ok(Wanted::One(ProviderId::Codex)),
        "antigravity" | "agy" | "google" | "gemini" => Ok(Wanted::One(ProviderId::Antigravity)),
        _ => Err(format!(
            "Unknown provider '{id}'. Known providers: {}, all.",
            PROVIDER_IDS.join(", ")
        )),
    }
}

pub fn newest(paths: &[PathBuf]) -> Option<PathBuf> {
    paths
        .iter()
        .filter(|p| p.exists())
        .max_by_key(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        })
        .cloned()
}

pub fn spec(id: ProviderId) -> Provider {
    match id {
        ProviderId::Claude => {
            let dir = claude_config_dir();
            Provider {
                id,
                label: "Claude Code",
                cli: "claude",
                store: store_dir("KEBACC_SWITCH_ACCOUNTS", ".kebacc-switch-accounts"),
                cred_candidates: vec![dir.join(".credentials.json")],
                config_candidates: vec![home().join(".claude.json"), dir.join(".claude.json")],
                cred_label: "~/.claude/.credentials.json",
                uses_keychain: cfg!(target_os = "macos"),
                keychain_service: Some("Claude Code-credentials"),
            }
        }
        ProviderId::Codex => {
            let dir = match std::env::var_os("CODEX_HOME") {
                Some(d) if !d.is_empty() => PathBuf::from(d),
                _ => home().join(".codex"),
            };
            Provider {
                id,
                label: "Codex",
                cli: "codex",
                store: store_dir(
                    "KEBACC_SWITCH_CODEX_ACCOUNTS",
                    ".kebacc-switch-codex-accounts",
                ),
                cred_candidates: vec![dir.join("auth.json")],
                config_candidates: Vec::new(),
                cred_label: "~/.codex/auth.json",
                uses_keychain: false,
                keychain_service: None,
            }
        }
        ProviderId::Antigravity => {
            let dir = match std::env::var_os("ANTIGRAVITY_HOME") {
                Some(d) if !d.is_empty() => PathBuf::from(d),
                _ => {
                    let mut dir = home();
                    for part in AGY_SESSION_DIR {
                        dir.push(part);
                    }
                    dir
                }
            };
            Provider {
                id,
                label: "Antigravity",
                cli: "agy",
                store: store_dir(
                    "KEBACC_SWITCH_ANTIGRAVITY_ACCOUNTS",
                    ".kebacc-switch-antigravity-accounts",
                ),
                cred_candidates: vec![dir.join(AGY_TOKEN_FILE)],
                config_candidates: Vec::new(),
                cred_label: "~/.gemini/antigravity-cli/antigravity-oauth-token",
                uses_keychain: false,
                keychain_service: None,
            }
        }
    }
}

fn store_dir(env: &str, default: &str) -> PathBuf {
    match std::env::var_os(env) {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => home().join(default),
    }
}

impl Provider {
    pub fn cred_file(&self) -> PathBuf {
        newest(&self.cred_candidates).unwrap_or_else(|| self.cred_candidates[0].clone())
    }

    pub fn config_file(&self) -> PathBuf {
        newest(&self.config_candidates).unwrap_or_else(|| {
            self.config_candidates
                .first()
                .cloned()
                .unwrap_or_else(|| self.cred_file())
        })
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.store.join(".backups")
    }

    pub fn snapshot_path(&self, email: &str) -> PathBuf {
        let safe: String = email
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '@' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.store.join(format!("{safe}.json"))
    }
}

pub fn protect_file(path: &Path) {
    if !path.exists() {
        return;
    }
    restrict(path, false);
}

pub fn protect_dir(path: &Path) {
    protect_dir_once(path);
}

#[cfg(windows)]
pub fn protect_new_file(path: &Path) {
    let owned = path.parent().is_some_and(is_locked_down);
    if !owned {
        protect_file(path);
    }
}

#[cfg(not(windows))]
pub fn protect_new_file(path: &Path) {
    protect_file(path);
}

fn locked_down() -> &'static std::sync::Mutex<std::collections::HashSet<PathBuf>> {
    static DONE: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> =
        std::sync::OnceLock::new();
    DONE.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

#[cfg(windows)]
fn is_locked_down(dir: &Path) -> bool {
    locked_down()
        .lock()
        .map(|done| done.contains(dir))
        .unwrap_or(false)
}

pub fn protect_dir_once(dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    let Ok(mut done) = locked_down().lock() else {
        return;
    };
    if !done.insert(dir.to_path_buf()) {
        return;
    }
    let mark = dir.join(PROTECTED_MARK);
    if mark.exists() {
        return;
    }
    restrict(dir, true);
    let _ = std::fs::write(&mark, b"");
}

pub fn reprotect_dir(dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    restrict(dir, true);
    let _ = std::fs::write(dir.join(PROTECTED_MARK), b"");
}

#[cfg(windows)]
fn restrict(path: &Path, dir: bool) {
    let grant = if dir {
        "*S-1-3-4:(OI)(CI)F"
    } else {
        "*S-1-3-4:(F)"
    };
    let mut icacls = std::process::Command::new("icacls");
    let _ = crate::proc::hidden(&mut icacls)
        .arg(path)
        .args(["/inheritance:r", "/grant:r", grant])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(not(windows))]
fn restrict(path: &Path, dir: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if dir { 0o700 } else { 0o600 };
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
}

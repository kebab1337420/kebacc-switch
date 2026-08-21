use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Left in a directory once its permissions have been cut down, so the next
/// process does not spend an `icacls` finding out what this one already knows.
/// The status line redraws several times a minute and each draw is a process, so
/// without it that call happens over and over for no change at all.
const PROTECTED_MARK: &str = ".protected";

/// The only pool this build knows. The Claude half lives in its own plugin, on
/// its own branch, and speaks to `~/.claude/.credentials.json` through its own
/// binary; nothing here reads or writes it.
pub const PROVIDER_ID: &str = "antigravity";

/// Where the Antigravity CLI (`agy`) keeps the login it is signed in with: one
/// flat file holding the very same payload the IDE keeps in the operating
/// system's credential store. Swapping it is the whole switch for the CLI, and
/// `live::set_creds_raw` mirrors it into the credential store so the IDE that
/// reads from there follows along.
const AGY_SESSION_DIR: [&str; 2] = [".gemini", "antigravity-cli"];
const AGY_TOKEN_FILE: &str = "antigravity-oauth-token";

pub struct Provider {
    pub label: &'static str,
    pub cli: &'static str,
    pub store: PathBuf,
    pub cred_candidates: Vec<PathBuf>,
    pub cred_label: &'static str,
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

/// Claude Code's own directory: its settings, its hooks, its slash commands.
/// Nothing to do with the Claude *pool* — this plugin is a Claude Code plugin,
/// and this is where it is installed, whatever CLI's logins it switches.
pub fn claude_config_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home().join(".claude"),
    })
    .clone()
}

/// `-Provider` survives as an option because the slash commands and the hooks
/// pass it. Only this pool answers to it now, and a name that is not it says so
/// rather than being quietly taken for antigravity.
///
/// `all` is not a pool but the scope one shared session hook carries on a
/// machine where the Claude half is installed too. This build has one pool, so
/// `all` is that pool: the hook stays valid whichever binary reads it.
pub fn resolve(id: &str) -> Result<(), String> {
    let key = id.trim().to_ascii_lowercase();
    if key.is_empty() {
        return Ok(());
    }
    match key.as_str() {
        "antigravity" | "agy" | "google" | "gemini" | "all" | "every" | "*" => Ok(()),
        "claude" | "claude-code" | "claudecode" | "cc" | "anthropic" => Err(format!(
            "'{id}' is the Claude pool, which this build does not carry. Install kebacc for it; this one switches {PROVIDER_ID} only."
        )),
        "codex" | "openai" | "chatgpt" | "gpt" => Err(format!(
            "'{id}' is the Codex pool, which this build does not carry. Install kebacc-codex for it; this one switches {PROVIDER_ID} only."
        )),
        _ => Err(format!(
            "Unknown provider '{id}'. This build carries {PROVIDER_ID} only."
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

pub fn spec() -> Provider {
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
        label: "Antigravity",
        cli: "agy",
        store: store_dir(
            "KEBACC_SWITCH_ANTIGRAVITY_ACCOUNTS",
            ".kebacc-switch-antigravity-accounts",
        ),
        cred_candidates: vec![dir.join(AGY_TOKEN_FILE)],
        cred_label: "~/.gemini/antigravity-cli/antigravity-oauth-token",
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

/// The same, for a directory whose permissions are to be set again whatever the
/// marker says: `doctor -Protect` is the repair, and it has to run even where a
/// previous run left the mark behind.
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

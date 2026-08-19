use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The only pool this build knows. The Claude half lives in its own plugin, on
/// its own branch, and speaks to `~/.claude/.credentials.json` through its own
/// binary; nothing here reads or writes it.
pub const PROVIDER_ID: &str = "codex";

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
/// rather than being quietly taken for codex.
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
        "codex" | "openai" | "chatgpt" | "gpt" | "all" | "every" | "*" => Ok(()),
        "claude" | "claude-code" | "claudecode" | "cc" | "anthropic" => Err(format!(
            "'{id}' is the Claude pool, which this build does not carry. Install kebacc-switch for it; this one switches {PROVIDER_ID} only."
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
    let dir = match std::env::var_os("CODEX_HOME") {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => home().join(".codex"),
    };
    Provider {
        label: "Codex",
        cli: "codex",
        store: store_dir(
            "KEBACC_SWITCH_CODEX_ACCOUNTS",
            ".kebacc-switch-codex-accounts",
        ),
        cred_candidates: vec![dir.join("auth.json")],
        cred_label: "~/.codex/auth.json",
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
    if done.insert(dir.to_path_buf()) {
        restrict(dir, true);
    }
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

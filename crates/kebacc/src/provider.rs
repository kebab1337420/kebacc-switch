use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::branch::{self, ConfigAt};

const PROTECTED_MARK: &str = ".protected";

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ProviderId {
    Claude,
    Codex,
    Antigravity,
    Grok,
    OpenCode,
}

impl ProviderId {
    pub const ALL: [ProviderId; 5] = [
        ProviderId::Claude,
        ProviderId::Codex,
        ProviderId::Antigravity,
        ProviderId::Grok,
        ProviderId::OpenCode,
    ];

    pub fn index(self) -> usize {
        match self {
            ProviderId::Claude => 0,
            ProviderId::Codex => 1,
            ProviderId::Antigravity => 2,
            ProviderId::Grok => 3,
            ProviderId::OpenCode => 4,
        }
    }

    pub fn at(index: usize) -> Option<ProviderId> {
        Self::ALL.get(index).copied()
    }

    pub fn branch(self) -> &'static branch::Branch {
        branch::of(self)
    }

    pub fn as_str(self) -> &'static str {
        self.branch().key
    }

    pub fn seal_account(self) -> &'static str {
        self.branch().seal_account
    }
}

const _: () = assert!(ProviderId::ALL.len() == branch::BRANCHES.len());

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Wanted {
    ids: Vec<ProviderId>,
    all: bool,
    off: bool,
}

pub enum PoolName {
    All,
    Off,
    One(ProviderId),
}

impl Wanted {
    pub fn unspecified() -> Self {
        Self::default()
    }

    pub fn all() -> Self {
        Self {
            ids: Vec::new(),
            all: true,
            off: false,
        }
    }

    pub fn off() -> Self {
        Self {
            ids: Vec::new(),
            all: false,
            off: true,
        }
    }

    pub fn one(id: ProviderId) -> Self {
        Self {
            ids: vec![id],
            all: false,
            off: false,
        }
    }

    pub fn is_unspecified(&self) -> bool {
        !self.all && !self.off && self.ids.is_empty()
    }

    pub fn is_off(&self) -> bool {
        self.off
    }

    pub fn is_all(&self) -> bool {
        self.all || self.ids.len() == ProviderId::ALL.len()
    }

    pub fn add(&mut self, id: ProviderId) {
        self.off = false;
        if self.all {
            return;
        }
        if !self.ids.contains(&id) {
            self.ids.push(id);
            self.ids.sort_by_key(|pool| {
                ProviderId::ALL
                    .iter()
                    .position(|known| known == pool)
                    .unwrap_or(99)
            });
        }
        if self.ids.len() == ProviderId::ALL.len() {
            self.all = true;
            self.ids.clear();
        }
    }

    pub fn mark_all(&mut self) {
        self.all = true;
        self.off = false;
        self.ids.clear();
    }

    pub fn mark_off(&mut self) {
        *self = Self::off();
    }

    pub fn remove(&mut self, id: ProviderId) {
        if self.off {
            return;
        }
        if self.all {
            self.all = false;
            self.ids = ProviderId::ALL
                .iter()
                .copied()
                .filter(|pool| *pool != id)
                .collect();
            return;
        }
        self.ids.retain(|pool| *pool != id);
    }

    pub fn union(&self, other: &Wanted) -> Wanted {
        if self.off {
            return other.clone();
        }
        if other.off {
            return self.clone();
        }
        if self.is_all() || other.is_all() {
            return Wanted::all();
        }
        let mut out = self.clone();
        for id in &other.ids {
            out.add(*id);
        }
        out
    }

    pub fn minus(&self, other: &Wanted) -> Wanted {
        if other.is_all() || other.off || other.is_unspecified() {
            return Wanted::off();
        }
        let mut out = if self.is_unspecified() || self.is_all() {
            let mut every = Wanted::unspecified();
            for id in ProviderId::ALL {
                every.add(id);
            }
            every
        } else {
            self.clone()
        };
        for id in &other.ids {
            out.remove(*id);
        }
        if out.ids.is_empty() && !out.all {
            Wanted::off()
        } else {
            out
        }
    }

    pub fn apply_name(&mut self, name: PoolName) {
        match name {
            PoolName::All => self.mark_all(),
            PoolName::Off => self.mark_off(),
            PoolName::One(id) => self.add(id),
        }
    }

    pub fn ids(&self) -> Vec<ProviderId> {
        if self.off {
            Vec::new()
        } else if self.is_all() || self.is_unspecified() {
            ProviderId::ALL.to_vec()
        } else {
            self.ids.clone()
        }
    }

    pub fn exactly_one(&self) -> Option<ProviderId> {
        if self.off || self.all {
            return None;
        }
        if self.ids.len() == 1 {
            Some(self.ids[0])
        } else {
            None
        }
    }

    pub fn flag_of(id: ProviderId) -> &'static str {
        id.branch().flag
    }

    pub fn flags(&self) -> Vec<String> {
        if self.off {
            return vec!["-off".into()];
        }
        if self.is_all() || self.is_unspecified() {
            return Vec::new();
        }
        self.ids
            .iter()
            .map(|id| Self::flag_of(*id).to_string())
            .collect()
    }

    pub fn flag_clause(&self) -> String {
        let flags = self.flags();
        if flags.is_empty() {
            String::new()
        } else {
            format!(" {}", flags.join(" "))
        }
    }

    pub fn display(&self) -> String {
        if self.off {
            return "off".into();
        }
        if self.is_all() || self.is_unspecified() {
            return "all".into();
        }
        self.ids
            .iter()
            .map(|id| id.branch().flag.trim_start_matches('-'))
            .collect::<Vec<_>>()
            .join("+")
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

pub fn session_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let asked = match std::env::var_os("CLAUDE_CONFIG_DIR") {
            Some(dir) if !dir.is_empty() => PathBuf::from(dir),
            _ => return state_dir(),
        };
        if asked == home().join(".claude") {
            return state_dir();
        }
        let dir = state_dir()
            .join("sessions")
            .join(crate::pool::short_hash(&asked.to_string_lossy()));
        let _ = std::fs::create_dir_all(&dir);
        reprotect_dir(&dir);
        dir
    })
    .clone()
}

pub fn pool_flags() -> String {
    let flags: Vec<&str> = branch::BRANCHES.iter().map(|branch| branch.flag).collect();
    match flags.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
        _ => flags.join(""),
    }
}

pub fn parse_pool_name(raw: &str) -> Option<PoolName> {
    let key = raw
        .trim()
        .trim_start_matches('-')
        .replace(['-', '_'], "")
        .to_ascii_lowercase();
    if key.is_empty() {
        return None;
    }
    match key.as_str() {
        "all" | "every" => Some(PoolName::All),
        "off" | "none" | "no" => Some(PoolName::Off),
        other => branch::find(other)
            .and_then(ProviderId::at)
            .map(PoolName::One),
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
    let branch = branch::of(id);
    let dir = branch_home(branch);
    let cred = match branch.cred_path_env.and_then(std::env::var_os) {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => dir.join(branch.cred_file),
    };
    let config_candidates = branch
        .config_files
        .iter()
        .map(|at| match at {
            ConfigAt::Home(name) => home().join(name),
            ConfigAt::Dir(name) => dir.join(name),
        })
        .collect();
    Provider {
        id,
        label: branch.label,
        cli: branch.cli,
        store: store_dir(branch.store_env, branch.store_default),
        cred_candidates: vec![cred],
        config_candidates,
        cred_label: branch.cred_label,
        uses_keychain: branch.keychain_on_macos && cfg!(target_os = "macos"),
        keychain_service: branch.keychain_service,
    }
}

fn branch_home(branch: &branch::Branch) -> PathBuf {
    if branch.home_env == "CLAUDE_CONFIG_DIR" {
        return claude_config_dir();
    }
    match std::env::var_os(branch.home_env) {
        Some(dir) if !dir.is_empty() => {
            let mut dir = PathBuf::from(dir);
            for part in branch.home_suffix {
                dir.push(part);
            }
            dir
        }
        _ => {
            let mut dir = home();
            for part in branch.home_default {
                dir.push(part);
            }
            dir
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

#[cfg(test)]
mod tests {
    use super::{parse_pool_name, PoolName, ProviderId, Wanted};

    #[test]
    fn short_and_full_names_are_the_same_pool() {
        assert!(matches!(
            parse_pool_name("-ag"),
            Some(PoolName::One(ProviderId::Antigravity))
        ));
        assert!(matches!(
            parse_pool_name("antigravity"),
            Some(PoolName::One(ProviderId::Antigravity))
        ));
        assert!(matches!(
            parse_pool_name("-cc"),
            Some(PoolName::One(ProviderId::Claude))
        ));
        assert!(matches!(
            parse_pool_name("-cl"),
            Some(PoolName::One(ProviderId::Claude))
        ));
        assert!(matches!(
            parse_pool_name("-cx"),
            Some(PoolName::One(ProviderId::Codex))
        ));
    }

    #[test]
    fn generated_flags_use_the_short_name_only_when_the_full_one_is_long() {
        assert_eq!(Wanted::flag_of(ProviderId::Claude), "-claude");
        assert_eq!(Wanted::flag_of(ProviderId::Codex), "-codex");
        assert_eq!(Wanted::flag_of(ProviderId::Antigravity), "-ag");
        assert!(Wanted::all().flags().is_empty());
        assert_eq!(Wanted::one(ProviderId::Antigravity).flags(), vec!["-ag"]);
    }

    #[test]
    fn two_pools_stay_two_pools() {
        let mut wanted = Wanted::one(ProviderId::Claude);
        wanted.add(ProviderId::Antigravity);
        assert_eq!(wanted.display(), "claude+ag");
        assert_eq!(wanted.flags(), vec!["-claude", "-ag"]);
    }

    #[test]
    fn naming_every_pool_collapses_to_all() {
        let mut wanted = Wanted::unspecified();
        for id in ProviderId::ALL {
            wanted.add(id);
        }
        assert!(wanted.is_all());
        assert!(wanted.flags().is_empty());
    }

    #[test]
    fn every_branch_answers_to_its_own_name() {
        for id in ProviderId::ALL {
            let branch = id.branch();
            assert_eq!(branch.key, id.as_str());
            assert_eq!(ProviderId::at(id.index()), Some(id));
            for name in std::iter::once(&branch.key).chain(branch.aliases) {
                match parse_pool_name(name) {
                    Some(PoolName::One(found)) => assert_eq!(found, id, "{name}"),
                    _ => panic!("{name} names no pool"),
                }
            }
        }
    }
}

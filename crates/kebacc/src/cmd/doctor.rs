use super::Options;
use crate::jsonio;
use crate::live;
use crate::lock;
use crate::pool::{self, Pool, Trust};
use crate::provider::{self, Provider, ProviderId};
use crate::seal;
use crate::term::{say, Color};
use serde_json::Value;
use std::path::{Path, PathBuf};

const GIT_CACHE_KEEP_DAYS: u64 = 30;

const STALE_NAMES: [&str; 6] = [
    ".threads.state",
    ".watch.pid",
    ".watch.state",
    "watch.log",
    "watch-hidden.vbs",
    "relaunch.log",
];

struct Report {
    problems: u32,
}

impl Report {
    fn bad(&mut self, text: &str) {
        say(&format!("  ! {text}"), Color::Red);
        self.problems += 1;
    }
    fn warn(&self, text: &str) {
        say(&format!("  ~ {text}"), Color::Yellow);
    }
    fn good(&self, text: &str) {
        say(&format!("  . {text}"), Color::Dim);
    }
}

pub fn run(provider: &Provider, opts: &Options) -> i32 {
    let mut report = Report { problems: 0 };

    say(
        &format!("{} — {}", provider.label, provider.store.display()),
        Color::Cyan,
    );
    say(
        &format!(
            "  kebacc {} on {} {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ),
        Color::Dim,
    );
    if let Some(marker) = super::update::installed_version() {
        if marker != env!("CARGO_PKG_VERSION") {
            say(
                &format!(
                    "  ! the plugin next to this binary says {marker}: reinstall so the two match"
                ),
                Color::Yellow,
            );
        }
    }

    if opts.rollback {
        let Some(backup) = live::newest_backup(provider) else {
            say("  No backup to roll back to.", Color::Yellow);
            return 1;
        };
        let put_back = lock::locked(lock::CRED_SWAP, || {
            live::set_creds_raw(provider, &backup.raw)
        });
        match put_back {
            Ok(Ok(())) => {}
            Ok(Err(problem)) => {
                say(&format!("  {problem}"), Color::Red);
                return 1;
            }
            Err(problem) => {
                say(&format!("  {problem}"), Color::Red);
                return 1;
            }
        }
        say(
            &format!(
                "  Put the credentials from {} back. The email in the config was left as it is.",
                backup.at.format("%Y-%m-%d %H:%M:%S")
            ),
            Color::Green,
        );
        return 0;
    }

    if let Some(shim) = install_dir()
        .map(|dir| dir.join("shim"))
        .filter(|s| s.exists())
    {
        report.warn(&format!(
            "An earlier version left {} on this machine. Remove it once nothing on your PATH points at it.",
            shim.display()
        ));
    }

    if let Some(dir) = install_dir() {
        if let Some((from, to, age)) = super::update::last(&dir) {
            report.good(&format!("updated from {from} to {to} {}", ago(age)));
        }
    }

    let backend = seal::backend();
    if backend == seal::Backend::None {
        report.warn("No OS secret store: snapshots cannot be encrypted on this machine.");
    } else {
        report.good(&format!("credentials sealed with {}", backend.name()));
    }

    if on_path(provider.cli) {
        report.good(&format!("{} found on PATH", provider.cli));
    } else {
        report.warn(&format!("{} is not on PATH.", provider.cli));
    }

    match live::creds_raw(provider) {
        Some(_) => {
            let email = live::identity(provider).and_then(|id| jsonio::str_of(&id, "emailAddress"));
            match email {
                Some(email) => report.good(&format!("logged in as {email}")),
                None => report.good(&format!("logged in ({})", provider.cred_label)),
            }
        }
        None => report.warn(&format!("not logged in ({})", provider.cred_label)),
    }

    if provider.id == ProviderId::Claude {
        check_settings(&mut report);
    }

    if !provider.store.exists() {
        report.warn("No pool directory yet. Nothing has been saved.");
        return exit_code(&report);
    }

    let store = Pool::new(provider);
    if store.key(false).is_none() {
        report.warn("The pool key is missing or unreadable: nothing can be verified.");
    }

    if opts.protect {
        provider::reprotect_dir(&provider.store);
        provider::reprotect_dir(&provider::state_dir());
    }

    let mut plain = 0u32;
    for entry in store.entries() {
        let name = entry.file_name();
        say(&format!("  {}", entry.email), Color::Plain);

        if opts.protect && !entry.protected && entry.creds.is_some() {
            let saved = jsonio::str_of(&entry.snapshot, "savedAt");
            let (sealed, _) = pool::new_snapshot(
                &entry.email,
                entry.creds.as_deref().unwrap_or_default(),
                entry.identity.as_ref(),
                entry.cache.as_ref(),
                saved.as_deref(),
            );
            if jsonio::write(&entry.file, &sealed).is_ok() {
                store.register(&name, &sealed);
                report.good("sealed");
            } else {
                report.bad("could not be sealed");
            }
            continue;
        }
        if opts.adopt && entry.trust != Trust::Trusted {
            if store.register(&name, &entry.snapshot) {
                report.good("stamped");
            } else {
                report.warn("cannot be stamped: no stable account id");
            }
            continue;
        }

        if entry.creds.is_none() {
            report.bad("the credentials cannot be read back");
        } else if !entry.protected {
            report.warn("stored in plain text");
            plain += 1;
        }
        if entry.trust == Trust::Changed {
            report.bad("CHANGED since it was registered");
        } else if entry.trust != Trust::Trusted {
            report.warn(entry.trust.verdict().0);
        }
        if entry.creds.is_some() && entry.protected && entry.trust == Trust::Trusted {
            report.good("sealed and stamped");
        }
        if let Some(raw) = entry.creds.as_deref() {
            token_report(provider, &entry, raw, opts.renew, &mut report);
        }
    }

    let log = crate::log::path();
    if log.exists() {
        report.good(&format!("switches are written down in {}", log.display()));
    }

    let stale = stale_files(provider);
    if !stale.is_empty() {
        if opts.clean {
            let mut removed = 0;
            for file in &stale {
                if std::fs::remove_file(file).is_ok() {
                    removed += 1;
                }
            }
            report.good(&format!(
                "removed {removed} leftover file(s) from an earlier version"
            ));
        } else {
            say(
                &format!(
                    "  {} leftover file(s) from an earlier version. Remove with: kebacc doctor -Provider {} -Clean",
                    stale.len(),
                    provider.id.as_str()
                ),
                Color::Yellow,
            );
        }
    }

    if plain > 0 && !opts.protect {
        say(
            &format!(
                "  {plain} snapshot(s) in plain text. Fix with: kebacc doctor -Provider {} -Protect",
                provider.id.as_str()
            ),
            Color::Yellow,
        );
    }
    exit_code(&report)
}

/// What the saved pair is worth, and, when asked, a new one in its place.
fn token_report(
    provider: &Provider,
    entry: &pool::Entry,
    raw: &str,
    renew: bool,
    report: &mut Report,
) {
    let expiry = crate::log::moment(crate::oauth::expires_at(raw));
    if !crate::oauth::stale(raw) {
        report.good(&format!("token good until {expiry}"));
        return;
    }
    if !renew {
        report.warn(&format!(
            "its token ran out at {expiry}. The switch renews it on the way in; renew it now with: kebacc doctor -Renew"
        ));
        return;
    }
    match crate::oauth::renew(raw) {
        Ok(fresh) if pool::save_creds(provider, &entry.file, &fresh) => report.good(&format!(
            "token renewed, good until {}",
            crate::log::moment(crate::oauth::expires_at(&fresh))
        )),
        Ok(_) => report.bad("the new token could not be written back to the pool"),
        Err(problem) => report.bad(&format!(
            "the token could not be renewed ({problem}). Run /login on this account and save it again"
        )),
    }
}

fn exit_code(report: &Report) -> i32 {
    if report.problems > 0 {
        1
    } else {
        0
    }
}

fn ago(age_ms: u128) -> String {
    let minutes = age_ms / 60_000;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    format!("{}h ago", minutes / 60)
}

fn install_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

fn check_settings(report: &mut Report) {
    let path = crate::provider::claude_config_dir().join("settings.json");
    let settings = jsonio::read(&path).unwrap_or(Value::Null);

    let line = settings
        .get("statusLine")
        .and_then(|l| jsonio::str_of(l, "command"))
        .unwrap_or_default();
    if line.contains("statusline") && is_ours_binary(&line) {
        match missing_path(&line) {
            Some(gone) => report.bad(&format!(
                "the status line points at {gone}, which is not there"
            )),
            None => report.good("status line installed"),
        }
    } else {
        say(
            "  status line not installed. Add it with: kebacc install -StatusLine",
            Color::Dim,
        );
    }

    let hooks = auto_hooks(&settings);
    match hooks.len() {
        0 => say(
            "  auto does not run on its own. Arm it with: kebacc install -AutoSwitch all",
            Color::Dim,
        ),
        1 => report.good(&format!(
            "auto runs at every session start, for {}",
            hook_scope(&hooks[0]).unwrap_or_else(|| "claude".into())
        )),
        count => report.warn(&format!(
            "{count} session hooks run auto. Reinstall with -AutoSwitch to leave one."
        )),
    }

    let midtask = midtask_hooks(&settings);
    match midtask.len() {
        0 => report.warn(
            "auto only runs between sessions. Re-arm it to also run mid-task: /kebacc-auto-claude",
        ),
        1 => report.good("auto also runs mid-task, before each tool call"),
        count => report.warn(&format!(
            "{count} mid-task hooks run auto. Re-arm it to leave one: /kebacc-auto-claude"
        )),
    }

    // The watcher covers what no hook can: a turn with no tool call in it. It
    // is started by the hooks and dies with the session, so its absence is
    // worth a line but never an alarm.
    if super::watch::on_duty() {
        report.good(&format!(
            "a watcher is on duty, checking every {}s even with no tool call",
            super::watch::interval().as_secs()
        ));
    } else {
        report.good("no watcher running yet. The next hook starts one.");
    }
}

pub fn auto_hooks(settings: &Value) -> Vec<String> {
    hooks_at(settings, "SessionStart")
}

/// The mid-task half: the `PreToolUse` hook that lets auto act while Claude is
/// in the middle of a run, instead of waiting for the next session.
pub fn midtask_hooks(settings: &Value) -> Vec<String> {
    hooks_at(settings, "PreToolUse")
}

fn hooks_at(settings: &Value, event: &str) -> Vec<String> {
    let mut found = Vec::new();
    let Some(groups) = settings
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(Value::as_array)
    else {
        return found;
    };
    for group in groups {
        let Some(hooks) = group.get("hooks").and_then(Value::as_array) else {
            continue;
        };
        for hook in hooks {
            let Some(command) = hook.get("command").and_then(Value::as_str) else {
                continue;
            };
            if is_auto_command(command) {
                found.push(command.to_string());
            }
        }
    }
    found
}

/// The program a command line names, without a path or `.exe`.
fn program_stem(command: &str) -> Option<String> {
    let word = quoted_words(command).into_iter().next()?;
    let name = word
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&word)
        .to_lowercase();
    Some(name.strip_suffix(".exe").unwrap_or(&name).to_string())
}

/// Whether this command line names our binary, including the name it had
/// before the rename, the scripts that came before that, and the leftover
/// `kebacc-codex` / `kebacc-antigravity` binaries from when each pool had
/// its own process.
pub fn is_ours_binary(command: &str) -> bool {
    let lower = command.to_lowercase();
    if lower.contains("claude-c") {
        return true;
    }
    matches!(
        program_stem(command).as_deref(),
        Some("kebacc") | Some("kebacc-switch") | Some("kebacc-codex") | Some("kebacc-antigravity")
    )
}

pub fn is_auto_command(command: &str) -> bool {
    let lower = command.to_lowercase();
    is_ours_binary(command) && lower.split_whitespace().any(|word| word == "auto")
}

pub fn hook_scope(command: &str) -> Option<String> {
    let words: Vec<&str> = command.split_whitespace().collect();
    let at = words.iter().position(|w| {
        w.eq_ignore_ascii_case("-provider") || w.eq_ignore_ascii_case("--provider")
    })?;
    words
        .get(at + 1)
        .map(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
}

fn missing_path(command: &str) -> Option<String> {
    quoted_words(command)
        .into_iter()
        .filter(|word| word.contains('/') || word.contains('\\'))
        .find(|word| !Path::new(word).exists())
}

pub fn quoted_words(command: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quote: Option<char> = None;
    for c in command.chars() {
        match quote {
            Some(open) if c == open => quote = None,
            Some(_) => word.push(c),
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ' ' => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            None => word.push(c),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

fn stale_files(provider: &Provider) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = pool::snapshot_files(&provider.store)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .map(|n| STALE_NAMES.contains(&n.to_string_lossy().as_ref()))
                .unwrap_or(false)
        })
        .collect();
    out.extend(
        pool::snapshot_files(&provider.backup_dir())
            .into_iter()
            .filter(|p| p.to_string_lossy().ends_with(".json.bak")),
    );
    out.extend(spent_git_caches(&provider::state_dir()));
    out
}

fn spent_git_caches(state: &Path) -> Vec<PathBuf> {
    let Ok(dir) = std::fs::read_dir(state) else {
        return Vec::new();
    };
    dir.filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                return false;
            };
            if !name.ends_with(".txt") {
                return false;
            }
            if name.starts_with("git-") {
                return true;
            }
            name.starts_with("gitstat-") && untouched_for(path, GIT_CACHE_KEEP_DAYS)
        })
        .collect()
}

fn untouched_for(path: &Path, days: u64) -> bool {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|at| at.elapsed().ok())
        .is_some_and(|age| age.as_secs() > days * 24 * 60 * 60)
}

fn on_path(cli: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".EXE;.CMD;.BAT".into())
            .split(';')
            .map(|e| e.to_lowercase())
            .collect()
    } else {
        vec![String::new()]
    };
    std::env::split_paths(&paths).any(|dir| {
        extensions
            .iter()
            .any(|ext| dir.join(format!("{cli}{ext}")).exists())
    })
}

#[cfg(test)]
mod tests {
    use super::{is_ours_binary, quoted_words};

    #[test]
    fn a_quoted_path_with_a_space_stays_one_word() {
        let words = quoted_words("\"C:/Program Files/kebacc.exe\" statusline");
        assert_eq!(words[0], "C:/Program Files/kebacc.exe");
        assert_eq!(words[1], "statusline");
    }

    #[test]
    fn bare_words_split_on_spaces() {
        assert_eq!(quoted_words("a b  c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn leftover_pool_binaries_are_still_ours() {
        assert!(is_ours_binary(
            "\"/tmp/kebacc\" auto -Provider claude -Hook"
        ));
        assert!(is_ours_binary(
            "\"C:/Users/me/.claude-tools/kebacc-switch.exe\" auto -Provider claude -Hook"
        ));
        assert!(is_ours_binary(
            "\"/tmp/kebacc-codex\" auto -Provider codex -Hook"
        ));
        assert!(is_ours_binary(
            "\"/tmp/kebacc-antigravity\" auto -Provider antigravity -Hook"
        ));
        assert!(!is_ours_binary("starship prompt"));
    }
}

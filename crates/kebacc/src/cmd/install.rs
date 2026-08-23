use super::Options;
use crate::term::{say, Color};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const COMMANDS: &[(&str, &str)] = &[
    (
        "kebacc-add.md",
        include_str!("../../../../plugins/kebacc/src/commands/kebacc-add.md"),
    ),
    (
        "kebacc-auto.md",
        include_str!("../../../../plugins/kebacc/src/commands/kebacc-auto.md"),
    ),
    (
        "kebacc-doctor.md",
        include_str!("../../../../plugins/kebacc/src/commands/kebacc-doctor.md"),
    ),
    (
        "kebacc-list.md",
        include_str!("../../../../plugins/kebacc/src/commands/kebacc-list.md"),
    ),
    (
        "kebacc-remove.md",
        include_str!("../../../../plugins/kebacc/src/commands/kebacc-remove.md"),
    ),
    (
        "kebacc-set.md",
        include_str!("../../../../plugins/kebacc/src/commands/kebacc-set.md"),
    ),
    (
        "kebacc-status.md",
        include_str!("../../../../plugins/kebacc/src/commands/kebacc-status.md"),
    ),
    (
        "kebacc-switch.md",
        include_str!("../../../../plugins/kebacc/src/commands/kebacc-switch.md"),
    ),
    (
        "kebacc-update.md",
        include_str!("../../../../plugins/kebacc/src/commands/kebacc-update.md"),
    ),
];

pub const LEGACY: &[&str] = &[
    "claude-cc.ps1",
    "claude-cc-core.ps1",
    "claude-cc-usage.ps1",
    "claude-cc-pool.ps1",
    "claude-cc-statusline.ps1",
    "claude-cc-providers.ps1",
    "kebacc-switch.ps1",
    "kebacc-switch",
    "kebacc-switch.exe",
    "kebacc-codex",
    "kebacc-codex.exe",
    "kebacc-antigravity",
    "kebacc-antigravity.exe",
    "statusline.js",
    "claude-cc.js",
    "package.json",
    "install-codex.ps1",
    ".codex-version",
    ".antigravity-version",
];

const DEAD_COMMANDS: &[&str] = &["refresh-a.md", "refresh-t.md"];

pub const RETIRED: &[&str] = &[
    "kebacc-install-codex.md",
    "kebacc-add-antigravity.md",
    "kebacc-add-claude.md",
    "kebacc-add-codex.md",
    "kebacc-auto-all.md",
    "kebacc-auto-antigravity.md",
    "kebacc-auto-claude.md",
    "kebacc-auto-codex.md",
    "kebacc-auto-toggle.md",
    "kebacc-doctor-antigravity.md",
    "kebacc-doctor-codex.md",
    "kebacc-list-all.md",
    "kebacc-list-antigravity.md",
    "kebacc-list-claude.md",
    "kebacc-list-codex.md",
    "kebacc-remove-antigravity.md",
    "kebacc-remove-claude.md",
    "kebacc-remove-codex.md",
    "kebacc-switch-antigravity.md",
    "kebacc-switch-claude.md",
    "kebacc-switch-codex.md",
    "kebacc-update-antigravity.md",
    "kebacc-update-codex.md",
];

pub const VERSION_FILE: &str = ".version";

pub const PROFILE_MARKER: &str = "# kebacc account switcher";

pub const LEGACY_PROFILE_MARKER: &str = "# kebacc-switch account switcher";

pub fn exe_name() -> String {
    format!("kebacc{}", std::env::consts::EXE_SUFFIX)
}

pub fn tools_dir(opts: &Options) -> PathBuf {
    opts.tools_dir
        .as_deref()
        .filter(|dir| !dir.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::provider::home().join(".claude-tools"))
}

pub fn run(opts: &Options) -> i32 {
    let tools = tools_dir(opts);
    let entry = tools.join(exe_name());

    let source = match source_binary(opts) {
        Ok(path) => path,
        Err(problem) => {
            say(&problem, Color::Red);
            return 1;
        }
    };

    if let Err(problem) = std::fs::create_dir_all(&tools) {
        say(
            &format!("Could not create {}: {problem}", tools.display()),
            Color::Red,
        );
        return 1;
    }

    for name in LEGACY {
        let stale = tools.join(name);
        if stale.is_file() {
            let _ = std::fs::remove_file(&stale);
        }
    }

    match place(&source, &entry) {
        Ok(true) => say(
            &format!("Installed kebacc into {}", tools.display()),
            Color::Green,
        ),
        Ok(false) => say(
            &format!("kebacc is already at {}", entry.display()),
            Color::Dim,
        ),
        Err(problem) => {
            say(&problem, Color::Red);
            return 1;
        }
    }

    match commands() {
        Ok(dir) => say(
            &format!("Installed the slash commands into {}", dir.display()),
            Color::Green,
        ),
        Err(problem) => {
            say(&problem, Color::Red);
            return 1;
        }
    }

    let version = super::update::version();

    let Some(installed) = reported_version(&entry) else {
        say(
            &format!("Copied the binary, but {} would not run.", entry.display()),
            Color::Red,
        );
        say(
            "The slash commands are in place; the settings were left untouched.",
            Color::Yellow,
        );
        say(
            "A security product blocking it is the usual reason. Allow it, then run this again.",
            Color::Yellow,
        );
        return 1;
    };
    if installed != version {
        say(
            &format!("The binary reports {installed} and this installer is {version}."),
            Color::Yellow,
        );
    }

    let _ = crate::jsonio::write_text(&tools.join(VERSION_FILE), &installed);

    if !opts.no_profile_edit {
        profile(&entry);
    }

    if opts.statusline.is_some() || opts.updates.is_some() {
        let mut call = vec!["wire", "-Quiet"];
        match opts.statusline {
            Some(true) => call.push("-StatusLine"),
            Some(false) => call.push("-NoStatusLine"),
            None => {}
        }
        match opts.updates {
            Some(true) => call.push("-AutoUpdate"),
            Some(false) => call.push("-NoAutoUpdate"),
            None => {}
        }
        if !ran(&entry, &call) {
            say("Could not write the Claude Code settings.", Color::Red);
            return 1;
        }
        if opts.statusline == Some(true) {
            say(
                "Pointed the Claude Code status line at the switcher",
                Color::Green,
            );
        }
        if opts.updates == Some(false) {
            say(
                "The switcher will not update itself: KEBACC_SWITCH_UPDATE=off",
                Color::Yellow,
            );
        }
    }

    if opts.auto_switch {
        let mut args = vec!["arm".to_string()];
        args.extend(opts.wanted.flags());
        if opts.wanted.exactly_one().is_some() {
            args.push("-Merge".into());
        }
        args.push("-Quiet".into());
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        if !ran(&entry, &args) {
            say("Could not arm the auto-switch.", Color::Red);
            return 1;
        }
        say(
            "Session start and every tool call now check the quota.",
            Color::Green,
        );
    }

    say("", Color::Plain);
    say(&format!("kebacc {version} is installed."), Color::Green);
    say("  kebacc add -claude|-codex|-ag", Color::Dim);
    say("  kebacc list      every pool, or list -ag", Color::Dim);
    say("  kebacc doctor    check everything", Color::Dim);
    0
}

fn source_binary(opts: &Options) -> Result<PathBuf, String> {
    if let Some(given) = opts
        .binary
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        let path = PathBuf::from(given);
        if !path.is_file() {
            return Err(format!("No binary at {given}."));
        }
        return std::fs::canonicalize(&path).map_err(|problem| format!("{given}: {problem}"));
    }
    let exe = std::env::current_exe().map_err(|_| "Cannot find my own path.".to_string())?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

fn place(source: &Path, entry: &Path) -> Result<bool, String> {
    if same_file(source, entry) {
        return Ok(false);
    }
    super::watch::request_stop();
    let fresh = entry.with_extension(format!("{}.new", std::process::id()));
    let _ = std::fs::remove_file(&fresh);
    std::fs::copy(source, &fresh).map_err(|problem| {
        format!(
            "Could not write into {}: {problem}",
            entry.parent().unwrap_or(entry).display()
        )
    })?;
    runnable(&fresh);
    let outcome = super::update::swap(entry, &fresh);
    if outcome.is_err() {
        let _ = std::fs::remove_file(&fresh);
    }
    outcome.map(|()| true)
}

fn same_file(one: &Path, two: &Path) -> bool {
    match (std::fs::canonicalize(one), std::fs::canonicalize(two)) {
        (Ok(one), Ok(two)) => one == two,
        _ => false,
    }
}

#[cfg(unix)]
fn runnable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn runnable(_path: &Path) {}

fn commands() -> Result<PathBuf, String> {
    let dir = crate::provider::claude_config_dir().join("commands");
    std::fs::create_dir_all(&dir).map_err(|problem| format!("{}: {problem}", dir.display()))?;
    sweep(&dir);
    for (name, body) in COMMANDS {
        let path = dir.join(name);
        std::fs::write(&path, body).map_err(|problem| format!("{}: {problem}", path.display()))?;
    }
    Ok(dir)
}

pub fn ours(name: &str) -> bool {
    if COMMANDS.iter().any(|(shipped, _)| *shipped == name) {
        return true;
    }
    if DEAD_COMMANDS.contains(&name) {
        return true;
    }
    if RETIRED.contains(&name) {
        return true;
    }
    name.ends_with(".md")
        && (name.starts_with("kebacc-")
            || name.starts_with("account-")
            || name.starts_with("claude-account-"))
}

fn sweep(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if ours(&name) && entry.path().is_file() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn reported_version(entry: &Path) -> Option<String> {
    let mut command = Command::new(entry);
    command.arg("--version");
    crate::proc::hidden(&mut command);
    let out = command.output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim().to_string();
    first.split_whitespace().last().map(str::to_string)
}

fn ran(entry: &Path, args: &[&str]) -> bool {
    let mut command = Command::new(entry);
    command.args(args);
    crate::proc::hidden(&mut command);
    command
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn profile(entry: &Path) {
    for path in profile_paths() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if has_profile_block(&existing) {
            let updated = repoint(&existing, entry, &path);
            if updated != existing && std::fs::write(&path, &updated).is_ok() {
                say(
                    &format!("Pointed kebacc in {} at the binary", path.display()),
                    Color::Green,
                );
            } else {
                say(
                    &format!("kebacc is already in {}.", path.display()),
                    Color::Dim,
                );
            }
            continue;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let block = format!("\n{PROFILE_MARKER}\n{}\n", function_line(entry, &path));
        if std::fs::write(&path, format!("{existing}{block}")).is_ok() {
            say(&format!("Added kebacc to {}", path.display()), Color::Green);
            say("Open a new shell for it to exist there.", Color::Dim);
        }
    }
}

pub fn has_profile_block(existing: &str) -> bool {
    existing.contains(PROFILE_MARKER) || existing.contains(LEGACY_PROFILE_MARKER)
}

pub fn is_profile_marker(line: &str) -> bool {
    let text = line.trim_start();
    text.starts_with(PROFILE_MARKER) || text.starts_with(LEGACY_PROFILE_MARKER)
}

fn repoint(existing: &str, entry: &Path, path: &Path) -> String {
    let wanted = function_line(entry, path);
    let mut out = String::new();
    let mut replace_next = false;
    for line in existing.lines() {
        if replace_next {
            replace_next = false;
            out.push_str(&wanted);
            out.push('\n');
            continue;
        }
        if is_profile_marker(line) {
            replace_next = true;
            out.push_str(PROFILE_MARKER);
            out.push('\n');
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn function_line(entry: &Path, profile: &Path) -> String {
    if profile
        .extension()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("ps1"))
    {
        format!("function kebacc {{ & \"{}\" @args }}", entry.display())
    } else {
        format!("kebacc() {{ \"{}\" \"$@\"; }}", entry.display())
    }
}

#[cfg(windows)]
pub fn profile_paths() -> Vec<PathBuf> {
    let Some(documents) = dirs::document_dir() else {
        return Vec::new();
    };
    let mut paths = vec![documents.join("PowerShell").join("profile.ps1")];
    let old = documents.join("WindowsPowerShell");
    if old.is_dir() {
        paths.push(old.join("profile.ps1"));
    }
    paths
}

#[cfg(not(windows))]
pub fn profile_paths() -> Vec<PathBuf> {
    let home = crate::provider::home();
    let shell = std::env::var("SHELL").unwrap_or_default();
    let rc = if shell.ends_with("/zsh") {
        ".zshrc"
    } else if shell.ends_with("/bash") {
        ".bashrc"
    } else {
        ".profile"
    };
    vec![home.join(rc)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_command_is_one_the_sweep_takes_back() {
        for (name, _) in COMMANDS {
            assert!(ours(name), "{name} is shipped but never removed");
        }
    }

    #[test]
    fn the_old_per_pool_commands_are_swept_not_shipped() {
        for name in [
            "kebacc-list-claude.md",
            "kebacc-list-codex.md",
            "kebacc-list-antigravity.md",
            "kebacc-list-all.md",
            "kebacc-auto-all.md",
            "kebacc-auto-toggle.md",
        ] {
            assert!(ours(name), "{name} must still be removed");
            assert!(
                !COMMANDS.iter().any(|(shipped, _)| *shipped == name),
                "{name} is retired and must not ship"
            );
        }
    }

    #[test]
    fn retired_commands_are_still_swept() {
        for name in RETIRED {
            assert!(ours(name), "{name} was shipped and must still be removed");
        }
        assert!(!COMMANDS
            .iter()
            .any(|(shipped, _)| RETIRED.contains(shipped)));
    }

    #[test]
    fn nothing_outside_this_toolkit_is_swept() {
        assert!(!ours("commit.md"));
        assert!(!ours("kebacc-list-claude.txt"));
        assert!(!ours("README.md"));
    }

    #[test]
    fn the_shipped_commands_carry_their_front_matter() {
        for (name, body) in COMMANDS {
            assert!(body.starts_with("---"), "{name} has no front matter");
        }
    }

    #[test]
    fn a_powershell_profile_gets_the_powershell_spelling() {
        let line = function_line(Path::new("C:/tools/kebacc.exe"), Path::new("p.ps1"));
        assert!(line.starts_with("function kebacc"));
        let line = function_line(Path::new("/tools/kebacc"), Path::new(".zshrc"));
        assert!(line.starts_with("kebacc()"));
    }

    #[test]
    fn repointing_replaces_the_line_under_the_marker_and_nothing_else() {
        let before =
            format!("keep me\n{PROFILE_MARKER}\nkebacc() {{ \"/old\" \"$@\"; }}\nkeep me too\n");
        let after = repoint(&before, Path::new("/new/kebacc"), Path::new(".zshrc"));
        assert!(after.contains("keep me\n"));
        assert!(after.contains("keep me too"));
        assert!(after.contains("/new/kebacc"));
        assert!(!after.contains("/old"));
    }

    #[test]
    fn a_legacy_profile_block_is_rewritten_to_the_new_name() {
        let before =
            format!("keep me\n{LEGACY_PROFILE_MARKER}\nkebacc-switch() {{ \"/old\" \"$@\"; }}\n");
        let after = repoint(&before, Path::new("/new/kebacc"), Path::new(".zshrc"));
        assert!(after.contains(PROFILE_MARKER));
        assert!(!after.contains(LEGACY_PROFILE_MARKER));
        assert!(after.contains("kebacc() { \"/new/kebacc\" \"$@\"; }"));
        assert!(!after.contains("kebacc-switch()"));
    }
}

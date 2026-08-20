//! Putting the switcher on this machine, which used to be `install.ps1` and
//! `install.sh`.
//!
//! The two scripts did the same nine things in the same order, in two
//! languages, and every change had to be made twice or the halves drifted.
//! They are one function now, and the binary that carries it is the thing being
//! installed: there is no separate installer to download, to keep in step with
//! the release, or to fail to find its own plugin directory.
//!
//! Nothing here downloads. Whoever runs this already has the binary — from a
//! release, from `cargo build --release`, from a clone — and this puts it, the
//! slash commands it carries, and the Claude Code settings it asks for into
//! place. Run it again to update: it overwrites what it owns and never touches
//! the pools.

use super::Options;
use crate::term::{say, Color};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The slash commands, carried inside the binary rather than copied out of the
/// directory the installer was run from. That directory only exists in a clone,
/// and the usual install has no clone in it.
///
/// Listed one by one because `include_str!` needs a literal. `ci.yml` checks
/// this list against the directory, so a command added there and forgotten here
/// fails the build instead of shipping as a command nobody gets.
pub const COMMANDS: &[(&str, &str)] = &[
    (
        "kebacc-add-antigravity.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-add-antigravity.md"),
    ),
    (
        "kebacc-auto-all.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-auto-all.md"),
    ),
    (
        "kebacc-auto-antigravity.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-auto-antigravity.md"),
    ),
    (
        "kebacc-auto-toggle.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-auto-toggle.md"),
    ),
    (
        "kebacc-doctor-antigravity.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-doctor-antigravity.md"),
    ),
    (
        "kebacc-list-all.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-list-all.md"),
    ),
    (
        "kebacc-list-antigravity.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-list-antigravity.md"),
    ),
    (
        "kebacc-remove-antigravity.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-remove-antigravity.md"),
    ),
    (
        "kebacc-switch-antigravity.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-switch-antigravity.md"),
    ),
    (
        "kebacc-update-antigravity.md",
        include_str!("../../../../plugins/kebacc-antigravity/src/commands/kebacc-update-antigravity.md"),
    ),
];

/// Versions before this one were a set of PowerShell scripts and a node status
/// line, dot-sourced from the tools directory. Left in place they would still be
/// on the PATH of a hook or a status line written by an earlier install.
///
/// Named one by one on purpose: the tools directory is shared — kebacc-codex
/// installs beside us — and a wildcard sweep would delete files this installer
/// never wrote.
pub const LEGACY: &[&str] = &[
    // The shell installers this half shipped before the installer moved into
    // the binary. The other halves' leftovers are theirs to sweep.
    "kebacc-antigravity.ps1",
    "install-antigravity.ps1",
    "uninstall-antigravity.ps1",
];

/// Two commands from a version that had a thread relauncher. Nothing answers
/// them any more, and they still show up in the slash command list.
const DEAD_COMMANDS: &[&str] = &["refresh-a.md", "refresh-t.md"];

/// The marker that says which version of the plugin is installed here.
pub const VERSION_FILE: &str = ".antigravity-version";

/// The line the shell profile is keyed on, so an uninstall can find its own
/// block and leave everything else in the file alone.
pub const PROFILE_MARKER: &str = "# kebacc-antigravity account switcher";

pub fn exe_name() -> String {
    format!("kebacc-antigravity{}", std::env::consts::EXE_SUFFIX)
}

/// Where the binary goes. `-ToolsDir` moves it, which is what the tests in
/// `ci.yml` use to keep a run out of the runner's real home directory.
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
            &format!("Installed kebacc-antigravity into {}", tools.display()),
            Color::Green,
        ),
        Ok(false) => say(
            &format!("kebacc-antigravity is already at {}", entry.display()),
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

    // The binary is asked what it is rather than taken on trust. A `-Binary`
    // pointing at an older build, a truncated download, and an executable a
    // security product refuses to start all fail here, where the message can
    // say so, instead of quietly disagreeing with the plugin for days.
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

    // The marker says what the binary answered, not what this installer thinks
    // it is: `-Binary` can hand over another build, and a marker that disagrees
    // with the file beside it sends `update` in a circle.
    let _ = crate::jsonio::write_text(&tools.join(VERSION_FILE), &installed);

    if !opts.no_profile_edit {
        profile(&entry);
    }

    // settings.json belongs to the user, and the installed binary is the one
    // that has to write its own path into it — not this process, which may be a
    // copy in a temporary directory that is about to disappear.
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
        // -Merge, not a plain arm: a hook already armed on a scope that covers
        // the other half too goes back with that scope intact. Installing this
        // plugin must not narrow what somebody else is running.
        if !ran(&entry, &["arm", "-Provider", crate::provider::PROVIDER_ID, "-Merge", "-Quiet"]) {
            say("Could not arm the auto-switch.", Color::Red);
            return 1;
        }
        say(
            "Session start and every tool call now check the quota.",
            Color::Green,
        );
    }

    say("", Color::Plain);
    say(
        &format!("kebacc-antigravity {version} is installed."),
        Color::Green,
    );
    say(
        "  kebacc-antigravity add       save the login you are on",
        Color::Dim,
    );
    say(
        "  kebacc-antigravity list      what is saved, and its quota",
        Color::Dim,
    );
    say("  kebacc-antigravity doctor    check everything", Color::Dim);
    0
}

/// The binary to install: the one named by `-Binary`, or the one running this,
/// which is the usual case now that the installer travels inside it.
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

/// Copies the binary in, and says whether anything moved. Installing over a
/// running copy is normal here — a status line runs several times a minute —
/// so the new one is written beside the old and swapped in, which Windows
/// allows where overwriting in place does not.
fn place(source: &Path, entry: &Path) -> Result<bool, String> {
    if same_file(source, entry) {
        return Ok(false);
    }
    // A watcher started by the binary being replaced would go on checking with
    // the old code for the rest of the session.
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

/// Writes the slash commands out, after taking away the names earlier versions
/// used so an update does not leave two of each.
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

/// Every name this plugin has ever installed a command under, which is also
/// every name it takes away. The other halves install into this same
/// directory, so a name carrying theirs is left where it is.
pub fn ours(name: &str) -> bool {
    if COMMANDS.iter().any(|(shipped, _)| *shipped == name) {
        return true;
    }
    if DEAD_COMMANDS.contains(&name) {
        return true;
    }
    let old = name.ends_with(".md")
        && (name.starts_with("kebacc-")
            || name.starts_with("account-")
            || name.starts_with("claude-account-"));
    old && !name.contains("codex") && !name.contains("claude")
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

/// `kebacc-antigravity` as a shell function rather than a directory on the PATH: it
/// is one line to add, and an earlier version of this toolkit put a `claude.exe`
/// shim on the PATH that nobody wants back.
fn profile(entry: &Path) {
    for path in profile_paths() {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if existing.contains(PROFILE_MARKER) {
            let updated = repoint(&existing, entry, &path);
            if updated != existing && std::fs::write(&path, &updated).is_ok() {
                say(
                    &format!("Pointed kebacc-antigravity in {} at the binary", path.display()),
                    Color::Green,
                );
            } else {
                say(
                    &format!("kebacc-antigravity is already in {}.", path.display()),
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
            say(
                &format!("Added kebacc-antigravity to {}", path.display()),
                Color::Green,
            );
            say("Open a new shell for it to exist there.", Color::Dim);
        }
    }
}

/// The old line ran the switcher through a script. Same marker, different body:
/// the line under the marker is replaced wherever it points.
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
        if line.trim_start().starts_with(PROFILE_MARKER) {
            replace_next = true;
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
        format!("function kebacc-antigravity {{ & \"{}\" @args }}", entry.display())
    } else {
        format!("kebacc-antigravity() {{ \"{}\" \"$@\"; }}", entry.display())
    }
}

/// The files a login shell reads. On Windows that is what
/// `$PROFILE.CurrentUserAllHosts` points at — under the user's Documents, which
/// OneDrive is free to have moved, so it is asked for rather than assembled out
/// of the home directory.
#[cfg(windows)]
pub fn profile_paths() -> Vec<PathBuf> {
    let Some(documents) = dirs::document_dir() else {
        return Vec::new();
    };
    let mut paths = vec![documents.join("PowerShell").join("profile.ps1")];
    // Windows PowerShell 5.1 reads a different file, and only gets the function
    // if it already has a profile directory: making one for a shell the user may
    // never open is not this installer's business.
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
    fn the_other_halves_commands_are_left_alone() {
        assert!(!ours("kebacc-list-codex.md"));
        assert!(!ours("kebacc-auto-codex.md"));
        assert!(!ours("kebacc-list-claude.md"));
        assert!(!ours("kebacc-add-claude.md"));
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
        let line = function_line(Path::new("C:/tools/kebacc-antigravity.exe"), Path::new("p.ps1"));
        assert!(line.starts_with("function kebacc-antigravity"));
        let line = function_line(Path::new("/tools/kebacc-antigravity"), Path::new(".zshrc"));
        assert!(line.starts_with("kebacc-antigravity()"));
    }

    #[test]
    fn repointing_replaces_the_line_under_the_marker_and_nothing_else() {
        let before = format!(
            "keep me\n{PROFILE_MARKER}\nkebacc-antigravity() {{ \"/old\" \"$@\"; }}\nkeep me too\n"
        );
        let after = repoint(&before, Path::new("/new/kebacc-antigravity"), Path::new(".zshrc"));
        assert!(after.contains("keep me\n"));
        assert!(after.contains("keep me too"));
        assert!(after.contains("/new/kebacc-antigravity"));
        assert!(!after.contains("/old"));
    }
}

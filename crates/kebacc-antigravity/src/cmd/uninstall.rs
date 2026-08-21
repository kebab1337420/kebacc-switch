//! Taking back what the install put down, which used to be `uninstall.ps1` and
//! `uninstall.sh`.
//!
//! The saved logins are left alone: they are the expensive thing to rebuild, a
//! reinstall finds them again, and removing a plugin is not a reason to lose
//! them. `-Pool` is how somebody says otherwise. So is anything in the tools
//! directory this plugin did not write, kebacc-codex included: every file is
//! named, and the directory itself only goes when nothing else is left in it.

use super::Options;
use crate::term::{ask, said_yes, say, Color};
use std::path::Path;
use std::time::Duration;

/// How long to wait for a watcher to notice it has been asked to stop. It looks
/// once a second, so this is generous.
const WATCHER_WAIT: Duration = Duration::from_secs(5);

/// How long the copy left behind waits for the name of the binary to come free.
const REAP_WAIT: Duration = Duration::from_secs(60);

pub fn run(opts: &Options) -> i32 {
    let tools = super::install::tools_dir(opts);

    if !opts.yes && !confirmed(&tools) {
        say("Nothing removed.", Color::Plain);
        return 0;
    }

    // The settings name a binary by path, and `wire` and `arm` recognise their
    // own work by the name in it rather than by where it lives. Uninstalling a
    // copy in a directory of its own would therefore unwire whatever install
    // the settings actually point at — usually the working one.
    let ours = settings_point_here(&tools);
    if ours {
        // Disarmed and unwired before the binary goes, or the hooks and the
        // status line are left pointing at a file that is no longer there — and
        // a hook left behind is worse than a stale setting: it fails at the
        // start of every session the user opens from here on.
        //
        // -Drop rather than off: it takes this pool out and leaves anything
        // else armed, where off disarms whatever it finds.
        super::arm::run(crate::provider::PROVIDER_ID, true, super::arm::Mode::Drop);
        super::wire::run(Some(false), None, true);
        // KEBACC_SWITCH_UPDATE is a setting about a binary that is about to be
        // gone, and left there it would silence the next install. The other
        // halves read the same name, so it stays while any of them is
        // installed: turning their updates back on is not this uninstaller's
        // call.
        if !super::install::sibling_installed(&tools) {
            super::wire::run(None, Some(true), true);
        }
    } else {
        say(
            "The Claude Code settings point at another install, so they were left alone.",
            Color::Yellow,
        );
    }

    // The watcher outlives the session that started it and answers to no hook,
    // so nothing above stops it. Left alone it would go on switching accounts
    // for half an hour after this says the switcher is gone. It belongs to
    // whichever install the hooks named, so it is only stopped when that is
    // this one.
    if ours {
        if !super::watch::stop_and_wait(WATCHER_WAIT) {
            say(
                "A watcher is still running. Close the sessions using it and run this again.",
                Color::Yellow,
            );
        } else {
            say("Stopped the background watcher.", Color::Green);
        }
    }

    let (removed, stuck) = files(&tools);
    if removed > 0 {
        say(
            &format!("Removed {removed} file(s) from {}", tools.display()),
            Color::Green,
        );
    }
    // On Windows the binary running this cannot delete itself, so the last of
    // it is handed to a copy that outlives this process. Everything the user is
    // told above has already happened; this is the file going after the message.
    let handed_over = stuck && reaper(&tools);
    if stuck && !handed_over {
        say(
            &format!(
                "{} could not be removed while it is running. Delete it once this shell exits.",
                tools.join(super::install::exe_name()).display()
            ),
            Color::Yellow,
        );
    }
    if handed_over {
        say(
            &format!(
                "{} goes as soon as this command exits.",
                tools.join(super::install::exe_name()).display()
            ),
            Color::Dim,
        );
    }
    if tools.is_dir() && !handed_over {
        if empty(&tools) {
            let _ = std::fs::remove_dir(&tools);
            say(&format!("Removed {}", tools.display()), Color::Green);
        } else {
            say(
                &format!("{} kept: another plugin has files there.", tools.display()),
                Color::Dim,
            );
        }
    }

    // The slash commands sit in one directory for every install of this, so
    // they belong to whichever one the settings name. Taking them out from
    // under a second install is how uninstalling the copy you did not want
    // breaks the one you did.
    if ours {
        let gone = commands();
        if gone > 0 {
            say(&format!("Removed {gone} slash command(s)"), Color::Green);
        }
        // With this half gone the -all pair has nothing to span unless another
        // half is still standing. The marker above is already off the disk, so
        // this counts what is left.
        super::install::sync_all_commands(&tools);
    }

    if !opts.no_profile_edit {
        profiles(&tools);
    }

    let pool = crate::provider::spec().store;
    if opts.pool {
        if pool.is_dir() {
            let _ = std::fs::remove_dir_all(&pool);
            say(
                &format!("Deleted the pool {}", pool.display()),
                Color::Yellow,
            );
        }
    } else if pool.is_dir() {
        say(
            &format!(
                "The saved accounts are still in {}. Delete them with -Pool.",
                pool.display()
            ),
            Color::Dim,
        );
    }

    say("", Color::Plain);
    if ours {
        say(
            "The status line and both hooks were taken out of the Claude Code settings.",
            Color::Dim,
        );
    } else {
        say(
            "The other install still owns the settings and the slash commands.",
            Color::Dim,
        );
    }
    0
}

/// Whether the Claude Code settings name the install being removed. Answers
/// true when nothing there mentions the switcher at all: there is nothing to
/// take out, and the calls below are what write the absence of it.
///
/// Compared as text, because that is what the settings hold: a command line
/// with a path quoted inside it. The separators are levelled and Windows is
/// matched without case, which is how that platform compares paths anyway.
fn settings_point_here(tools: &Path) -> bool {
    let path = crate::provider::claude_config_dir().join("settings.json");
    let Some(settings) = crate::jsonio::read(&path) else {
        return true;
    };
    let mut ours = Vec::new();
    if let Some(line) = settings
        .get("statusLine")
        .and_then(|line| line.get("command"))
    {
        if let Some(text) = line.as_str() {
            ours.push(text.to_string());
        }
    }
    if let Some(hooks) = settings.get("hooks").and_then(|hooks| hooks.as_object()) {
        for groups in hooks.values() {
            collect_commands(groups, &mut ours);
        }
    }
    let ours: Vec<String> = ours
        .into_iter()
        .filter(|text| text.contains("kebacc-antigravity"))
        .map(|text| level(&text))
        .collect();
    if ours.is_empty() {
        return true;
    }
    let wanted = level(&tools.join(super::install::exe_name()).display().to_string());
    ours.iter().any(|text| text.contains(&wanted))
}

/// Every `command` string under a hook event, however deeply the groups nest.
fn collect_commands(node: &serde_json::Value, out: &mut Vec<String>) {
    match node {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_commands(item, out);
            }
        }
        serde_json::Value::Object(fields) => {
            for (name, value) in fields {
                if name == "command" {
                    if let Some(text) = value.as_str() {
                        out.push(text.to_string());
                    }
                } else {
                    collect_commands(value, out);
                }
            }
        }
        _ => {}
    }
}

/// One spelling for a path that may arrive with either separator, and without
/// case on the platform that ignores it.
fn level(text: &str) -> String {
    let text = text.replace('\\', "/");
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

/// Starts the copy that finishes the job once this process is gone: on Windows
/// a running image cannot be deleted, whatever it is called, so the file this
/// very code runs from outlives the uninstall by however long the shell takes
/// to return. The copy lives in the temporary directory, waits for the name to
/// come free, takes it, and removes the directory if nothing else is left.
///
/// Answers whether one was started. False means the caller says so itself.
fn reaper(tools: &Path) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let stand_in = std::env::temp_dir().join(format!(
        "kebacc-antigravity-reap-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
    // The copy from the last uninstall is still there: it is the one file that
    // cannot delete itself. Whichever one runs next takes it.
    sweep_reapers();
    let _ = std::fs::remove_file(&stand_in);
    if std::fs::copy(&exe, &stand_in).is_err() {
        return false;
    }
    let mut command = std::process::Command::new(&stand_in);
    command
        .arg("reap")
        .arg("-ToolsDir")
        .arg(tools)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::proc::detach(&mut command);
    command.spawn().is_ok()
}

/// The other half of that: run from the copy, with nothing to do but wait for
/// the file to come free. It gives up after a while — a shell left open on the
/// binary is not worth a process that waits forever — and what it leaves behind
/// is what the next install sweeps anyway.
pub fn reap(opts: &Options) -> i32 {
    // -ToolsDir is not optional here, and the command is not in the help: this
    // is only ever reached by the copy an uninstall starts, which passes one.
    // Without the guard, somebody typing it by hand would delete a working
    // install from under themselves.
    if opts.tools_dir.is_none() {
        return 64;
    }
    let tools = super::install::tools_dir(opts);
    let entry = tools.join(super::install::exe_name());
    let deadline = std::time::Instant::now() + REAP_WAIT;
    while entry.exists() && std::time::Instant::now() < deadline {
        if std::fs::remove_file(&entry).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let _ = std::fs::remove_file(entry.with_extension("old"));
    let _ = std::fs::remove_file(tools.join(format!("{}.old", super::install::exe_name())));
    if tools.is_dir() && empty(&tools) {
        let _ = std::fs::remove_dir(&tools);
    }
    0
}

/// Old reaper copies in the temporary directory. Each one outlives the process
/// that ran it, so they are swept by the next one rather than by themselves.
fn sweep_reapers() {
    let temp = std::env::temp_dir();
    let Ok(entries) = std::fs::read_dir(&temp) else {
        return;
    };
    let mine = format!("kebacc-antigravity-reap-{}", std::process::id());
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("kebacc-antigravity-reap-") || name.starts_with(&mine) {
            continue;
        }
        // A copy younger than the wait it was given may still be sitting on a
        // binary somebody has open. Windows would refuse to delete it anyway;
        // this is so the same code does not unlink a working one on the
        // platforms that would allow it.
        let working = entry
            .metadata()
            .and_then(|about| about.modified())
            .and_then(|then| {
                std::time::SystemTime::now()
                    .duration_since(then)
                    .map_err(|_| std::io::Error::other("in the future"))
            })
            .is_ok_and(|since| since < REAP_WAIT);
        if !working {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Whether anything is left in the tools directory that is not ours. The binary
/// on its way out does not count: it is either gone already or about to be.
fn empty(tools: &Path) -> bool {
    let exe = super::install::exe_name();
    let ours = [
        exe.clone(),
        format!("{exe}.old"),
        "kebacc-antigravity.old".into(),
    ];
    let Ok(entries) = std::fs::read_dir(tools) else {
        return false;
    };
    entries
        .flatten()
        .all(|entry| ours.contains(&entry.file_name().to_string_lossy().to_string()))
}

fn confirmed(tools: &Path) -> bool {
    say(
        &format!(
            "This removes the switcher from {}, the slash commands and the profile function.",
            tools.display()
        ),
        Color::Plain,
    );
    say("Saved logins are not touched.", Color::Dim);
    said_yes(&ask("Continue? [y/N]"))
}

/// Named one by one rather than removing the whole directory: kebacc-codex
/// installs its binary into this same directory, and taking the directory would
/// uninstall a plugin nobody asked about.
fn files(tools: &Path) -> (usize, bool) {
    if !tools.is_dir() {
        return (0, false);
    }
    let exe = super::install::exe_name();
    let mut removed = 0;
    let mut named: Vec<String> = vec![
        exe.clone(),
        // Both spellings of the binary moved aside: `update` renames it by
        // replacing the extension, and the scripts this replaced wrote the
        // other one.
        format!("{exe}.old"),
        "kebacc-antigravity.old".to_string(),
        super::install::VERSION_FILE.to_string(),
        super::update::MARKER.to_string(),
        "update-antigravity.stamp".to_string(),
    ];
    named.extend(super::install::LEGACY.iter().map(|name| name.to_string()));
    let mut stuck = false;
    for name in &named {
        let path = tools.join(name);
        if !path.is_file() {
            continue;
        }
        if std::fs::remove_file(&path).is_ok() {
            removed += 1;
        } else if *name == exe {
            // Windows holds the name of a running image, and the binary is the
            // only file here that is ever running. Anything else refusing to go
            // is a permission problem, which a copy left behind cannot fix
            // either — it is reported by the directory that stays.
            stuck = true;
        }
    }
    // Half-finished updates: kebacc-antigravity.<pid>.new.
    if let Ok(entries) = std::fs::read_dir(tools) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("kebacc-antigravity.")
                && name.ends_with(".new")
                && std::fs::remove_file(entry.path()).is_ok()
            {
                removed += 1;
            }
        }
    }
    (removed, stuck)
}

fn commands() -> usize {
    let dir = crate::provider::claude_config_dir().join("commands");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 0;
    };
    let mut gone = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy().to_string();
        if super::install::ours(&name)
            && entry.path().is_file()
            && std::fs::remove_file(entry.path()).is_ok()
        {
            gone += 1;
        }
    }
    gone
}

/// The function is one block: the marker and the line under it. Only those two
/// go, and anything else in the profile stays.
fn profiles(tools: &Path) {
    let wanted = level(&tools.join(super::install::exe_name()).display().to_string());
    for path in super::install::profile_paths() {
        let Ok(existing) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !existing.contains(super::install::PROFILE_MARKER) {
            continue;
        }
        // The line under the marker names the binary it runs. One that names
        // another install is that install's line, and this is not the command
        // that gets to take it out.
        if !block_body(&existing)
            .iter()
            .any(|line| level(line).contains(&wanted))
        {
            say(
                &format!(
                    "Left the kebacc-antigravity line in {} alone.",
                    path.display()
                ),
                Color::Dim,
            );
            continue;
        }
        let kept = without_block(&existing);
        if std::fs::write(&path, &kept).is_ok() {
            say(
                &format!("Took kebacc-antigravity out of {}", path.display()),
                Color::Green,
            );
        }
    }
}

/// The lines under each marker: what the block actually runs.
fn block_body(existing: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut take_next = false;
    for line in existing.lines() {
        if take_next {
            out.push(line.to_string());
            take_next = false;
            continue;
        }
        if line
            .trim_start()
            .starts_with(super::install::PROFILE_MARKER)
        {
            take_next = true;
        }
    }
    out
}

fn without_block(existing: &str) -> String {
    let mut out = String::new();
    let mut skip_next = false;
    for line in existing.lines() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if line
            .trim_start()
            .starts_with(super::install::PROFILE_MARKER)
        {
            skip_next = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_block_goes_and_the_rest_of_the_file_stays() {
        let before = format!(
            "export EDITOR=vi\n{}\nkebacc-antigravity() {{ \"/x\" \"$@\"; }}\nalias ll='ls -l'\n",
            super::super::install::PROFILE_MARKER
        );
        let after = without_block(&before);
        assert!(after.contains("export EDITOR=vi"));
        assert!(after.contains("alias ll="));
        assert!(!after.contains("kebacc-antigravity"));
    }

    #[test]
    fn a_profile_without_our_block_is_left_exactly_as_it_was() {
        let before = "export EDITOR=vi\nalias ll='ls -l'\n";
        assert_eq!(without_block(before), before);
    }

    #[test]
    fn the_line_under_the_marker_is_the_one_read_back() {
        let profile = format!(
            "export PATH=/bin\n{}\nkebacc-antigravity() {{ /a/tools/kebacc-antigravity \"$@\"; }}\n",
            super::super::install::PROFILE_MARKER
        );
        assert_eq!(
            block_body(&profile)
                .iter()
                .filter(|line| line.contains("/a/tools/"))
                .count(),
            1
        );
        assert!(!block_body(&profile)
            .iter()
            .any(|line| line.contains("/b/tools/")));
    }

    #[test]
    fn a_path_is_levelled_to_one_spelling() {
        let one = level("C:\\Users\\a\\.claude-tools\\kebacc-antigravity.exe");
        let two = level("c:/users/a/.claude-tools/kebacc-antigravity.exe");
        if cfg!(windows) {
            assert_eq!(one, two);
        } else {
            assert!(one.contains("/kebacc-antigravity.exe"));
        }
    }

    #[test]
    fn the_commands_under_a_hook_event_are_all_found() {
        let hooks = serde_json::json!([
            { "hooks": [
                { "type": "command", "command": "one" },
                { "type": "command", "command": "two" }
            ] }
        ]);
        let mut out = Vec::new();
        collect_commands(&hooks, &mut out);
        assert_eq!(out, vec!["one".to_string(), "two".to_string()]);
    }

    #[test]
    fn a_directory_with_only_our_binary_in_it_counts_as_empty() {
        let dir = std::env::temp_dir().join(format!("kebacc-empty-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        assert!(empty(&dir));
        let _ = std::fs::write(dir.join(super::super::install::exe_name()), b"x");
        assert!(empty(&dir));
        let _ = std::fs::write(dir.join("somebody-elses-tool"), b"x");
        assert!(!empty(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

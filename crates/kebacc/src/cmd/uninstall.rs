use super::install::level;
use super::Options;
use crate::term::{ask, said_yes, say, Color};
use std::path::Path;
use std::time::Duration;

const WATCHER_WAIT: Duration = Duration::from_secs(5);

const REAP_WAIT: Duration = Duration::from_secs(60);

pub fn run(opts: &Options) -> i32 {
    let tools = super::install::tools_dir(opts);

    if !opts.yes && !confirmed(&tools) {
        say("Nothing removed.", Color::Plain);
        return 0;
    }

    let ours = settings_point_here(&tools);
    if ours {
        super::arm::run(&crate::provider::Wanted::off(), true, super::arm::Mode::Set);
        super::wire::run(Some(false), None, true);
        super::wire::run(None, Some(true), true);
    } else {
        say(
            "The Claude Code settings point at another install, so they were left alone.",
            Color::Yellow,
        );
    }

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

    if ours {
        let gone = commands();
        if gone > 0 {
            say(&format!("Removed {gone} slash command(s)"), Color::Green);
        }
    }

    if !opts.no_profile_edit {
        profiles(&tools);
    }

    if opts.pool {
        for id in crate::provider::ProviderId::ALL {
            let pool = crate::provider::spec(id).store;
            if pool.is_dir() {
                let _ = std::fs::remove_dir_all(&pool);
                say(
                    &format!("Deleted the pool {}", pool.display()),
                    Color::Yellow,
                );
            }
        }
    } else {
        for id in crate::provider::ProviderId::ALL {
            let pool = crate::provider::spec(id).store;
            if pool.is_dir() {
                say(
                    &format!(
                        "The saved accounts are still in {}. Delete them with -Pool.",
                        pool.display()
                    ),
                    Color::Dim,
                );
            }
        }
    }

    say("", Color::Plain);
    if ours {
        say(
            "The status line and both hooks were taken out of the Claude Code settings.",
            Color::Dim,
        );
    } else {
        say(
            "Another copy of the switcher still owns the settings and the slash commands.",
            Color::Dim,
        );
    }
    0
}

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
        .filter(|text| super::doctor::is_ours_binary(text))
        .map(|text| level(&text))
        .collect();
    if ours.is_empty() {
        return true;
    }
    let wanted = level(&tools.join(super::install::exe_name()).display().to_string());
    ours.iter().any(|text| text.contains(&wanted))
}

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

fn reaper(tools: &Path) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let stand_in = std::env::temp_dir().join(format!(
        "kebacc-reap-{}{}",
        std::process::id(),
        std::env::consts::EXE_SUFFIX
    ));
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
    crate::proc::spawn_detached(&mut command).is_ok()
}

pub fn reap(opts: &Options) -> i32 {
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

fn sweep_reapers() {
    let temp = std::env::temp_dir();
    let Ok(entries) = std::fs::read_dir(&temp) else {
        return;
    };
    let pid = std::process::id();
    let mine_new = format!("kebacc-reap-{pid}");
    let mine_old = format!("kebacc-switch-reap-{pid}");
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let ours = name.starts_with("kebacc-reap-") || name.starts_with("kebacc-switch-reap-");
        if !ours || name.starts_with(&mine_new) || name.starts_with(&mine_old) {
            continue;
        }
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

fn empty(tools: &Path) -> bool {
    let exe = super::install::exe_name();
    let ours = [
        exe.clone(),
        format!("{exe}.old"),
        "kebacc.old".into(),
        "kebacc-switch.old".into(),
        "kebacc-switch.exe".into(),
        "kebacc-switch".into(),
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

fn files(tools: &Path) -> (usize, bool) {
    if !tools.is_dir() {
        return (0, false);
    }
    let exe = super::install::exe_name();
    let mut removed = 0;
    let mut named: Vec<String> = vec![
        exe.clone(),
        format!("{exe}.old"),
        "kebacc.old".to_string(),
        "kebacc-switch.old".to_string(),
        super::install::VERSION_FILE.to_string(),
        super::update::MARKER.to_string(),
        "update.stamp".to_string(),
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
            stuck = true;
        }
    }
    if let Ok(entries) = std::fs::read_dir(tools) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let ours_new = (name.starts_with("kebacc.") || name.starts_with("kebacc-switch."))
                && name.ends_with(".new");
            if ours_new && std::fs::remove_file(entry.path()).is_ok() {
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

fn profiles(tools: &Path) {
    let wanted = level(&tools.join(super::install::exe_name()).display().to_string());
    for path in super::install::profile_paths() {
        let Ok(existing) = std::fs::read_to_string(&path) else {
            continue;
        };
        if !super::install::has_profile_block(&existing) {
            continue;
        }
        if !removable(&existing, &wanted) {
            say(
                &format!("Left the kebacc line in {} alone.", path.display()),
                Color::Dim,
            );
            continue;
        }
        let kept = without_block(&existing);
        if std::fs::write(&path, &kept).is_ok() {
            say(
                &format!("Took kebacc out of {}", path.display()),
                Color::Green,
            );
        }
    }
}

/// Whether this uninstall gets to take the block out. Ours, plainly. And one
/// naming a binary that is not on the machine any more, which is nobody's:
/// something took that binary without coming through here, and what is left is
/// a name that fails in every new shell with nothing installed to explain it.
fn removable(existing: &str, wanted: &str) -> bool {
    if block_body(existing)
        .iter()
        .any(|line| level(line).contains(wanted))
    {
        return true;
    }
    !super::install::block_line(existing)
        .and_then(|line| super::install::named_binary(&line))
        .is_some_and(|named| named.is_file())
}

fn block_body(existing: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut take_next = false;
    for line in existing.lines() {
        if take_next {
            out.push(line.to_string());
            take_next = false;
            continue;
        }
        if super::install::is_profile_marker(line) {
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
        if super::install::is_profile_marker(line) {
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
            "export EDITOR=vi\n{}\nkebacc() {{ \"/x\" \"$@\"; }}\nalias ll='ls -l'\n",
            super::super::install::PROFILE_MARKER
        );
        let after = without_block(&before);
        assert!(after.contains("export EDITOR=vi"));
        assert!(after.contains("alias ll="));
        assert!(!after.contains("kebacc()"));
    }

    #[test]
    fn a_legacy_profile_block_is_taken_out_too() {
        let before = format!(
            "export EDITOR=vi\n{}\nkebacc-switch() {{ \"/x\" \"$@\"; }}\nalias ll='ls -l'\n",
            super::super::install::LEGACY_PROFILE_MARKER
        );
        let after = without_block(&before);
        assert!(after.contains("export EDITOR=vi"));
        assert!(after.contains("alias ll="));
        assert!(!after.contains("kebacc-switch"));
    }

    #[test]
    fn a_profile_without_our_block_is_left_exactly_as_it_was() {
        let before = "export EDITOR=vi\nalias ll='ls -l'\n";
        assert_eq!(without_block(before), before);
    }

    #[test]
    fn the_line_under_the_marker_is_the_one_read_back() {
        let profile = format!(
            "export PATH=/bin\n{}\nkebacc() {{ /a/tools/kebacc \"$@\"; }}\n",
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
    fn a_block_naming_a_binary_that_is_gone_is_taken_out() {
        let dead = format!(
            "{}\nkebacc() {{ \"/gone/kebacc\" \"$@\"; }}\n",
            super::super::install::PROFILE_MARKER
        );
        assert!(removable(&dead, "/some/other/install/kebacc"));
    }

    #[test]
    fn a_block_naming_another_install_that_is_still_there_is_left_alone() {
        let here = std::env::current_exe().expect("a test binary has a path");
        let theirs = format!(
            "{}\nkebacc() {{ \"{}\" \"$@\"; }}\n",
            super::super::install::PROFILE_MARKER,
            here.display()
        );
        assert!(!removable(&theirs, "/some/other/install/kebacc"));
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

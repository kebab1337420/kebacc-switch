use crate::jsonio;
use crate::provider::{self, Wanted};
use crate::term::{say, Color};
use serde_json::{json, Value};
use std::path::PathBuf;

const SETTINGS: [&str; 2] = ["settings.json", "settings.local.json"];
const SESSION_START: &str = "SessionStart";
const PRE_TOOL_USE: &str = "PreToolUse";
const TIMEOUT: u64 = 10;
const MIDTASK_TIMEOUT: u64 = 10;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Set,
    Merge,
    Drop,
}

pub fn run(wanted: &Wanted, quiet: bool, mode: Mode) -> i32 {
    if wanted.is_off() && mode != Mode::Set {
        say(
            "-Merge and -Drop need a pool to add or take out, not 'off'.",
            Color::Red,
        );
        return 64;
    }
    if matches!(mode, Mode::Merge | Mode::Drop) && wanted.is_unspecified() {
        say(
            &format!(
                "-Merge and -Drop need a pool: {}.",
                crate::provider::pool_flags()
            ),
            Color::Red,
        );
        return 64;
    }

    let asked = if wanted.is_off() {
        None
    } else if wanted.is_unspecified() {
        Some(Wanted::all())
    } else {
        Some(wanted.clone())
    };

    let dir = provider::claude_config_dir();
    let mut touched = false;
    let mut written = asked.clone();

    for name in SETTINGS {
        let path = dir.join(name);
        let Some(mut settings) = jsonio::read(&path) else {
            continue;
        };
        let before = settings.clone();
        let existing = strip(&mut settings);
        if let (Some(asked), Some(command)) = (&asked, existing.first()) {
            let exe = canonical_exe(&exe_of(command));
            let had = crate::cmd::doctor::hook_wanted(command);
            let scope = match mode {
                Mode::Set => asked.clone(),
                Mode::Merge => had.union(asked),
                Mode::Drop => had.minus(asked),
            };
            if scope.is_off() {
                written = None;
            } else {
                arm_both(&mut settings, &exe, &scope);
                written = Some(scope);
            }
            touched = true;
        }
        if settings != before {
            if let Err(problem) = jsonio::write(&path, &settings) {
                say(
                    &format!("Could not write {}: {problem}", path.display()),
                    Color::Red,
                );
                return 1;
            }
        }
    }

    if let Some(scope) = &asked {
        if !touched && mode != Mode::Drop {
            let path = dir.join(SETTINGS[0]);
            let mut settings = jsonio::read(&path).unwrap_or_else(|| json!({}));
            strip(&mut settings);
            arm_both(&mut settings, &installed().to_string_lossy(), scope);
            if let Err(problem) = jsonio::write(&path, &settings) {
                say(
                    &format!("Could not write {}: {problem}", path.display()),
                    Color::Red,
                );
                return 1;
            }
        } else if !touched {
            written = None;
        }
    }

    if written.as_ref().is_none_or(Wanted::is_off) {
        crate::cmd::watch::request_stop();
    }

    if !quiet {
        match &written {
            Some(scope) if !scope.is_off() => println!("auto {}", scope.display()),
            _ => println!("auto off"),
        }
    }
    0
}

pub fn armed() -> Option<Wanted> {
    let dir = provider::claude_config_dir();
    let mut scope = Wanted::off();
    for name in SETTINGS {
        let Some(settings) = jsonio::read(&dir.join(name)) else {
            continue;
        };
        for command in crate::cmd::doctor::auto_hooks(&settings) {
            scope = scope.union(&crate::cmd::doctor::hook_wanted(&command));
        }
    }
    (!scope.is_off()).then_some(scope)
}

pub fn migrate() {
    let dir = provider::claude_config_dir();
    for name in SETTINGS {
        let path = dir.join(name);
        let Some(mut settings) = jsonio::read(&path) else {
            continue;
        };
        let before = settings.clone();
        rewrite_hooks(&mut settings);
        if settings != before {
            let _ = jsonio::write(&path, &settings);
        }
    }
}

fn arm_both(settings: &mut Value, exe: &str, scope: &Wanted) {
    add(
        settings,
        SESSION_START,
        None,
        &line(exe, scope, false),
        TIMEOUT,
    );
    add(
        settings,
        PRE_TOOL_USE,
        Some("*"),
        &line(exe, scope, true),
        MIDTASK_TIMEOUT,
    );
}

fn rewrite_hooks(settings: &mut Value) {
    for event in [SESSION_START, PRE_TOOL_USE] {
        let Some(groups) = settings
            .get_mut("hooks")
            .and_then(|hooks| hooks.get_mut(event))
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for group in groups {
            let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                continue;
            };
            for hook in list {
                let Some(command) = hook.get("command").and_then(Value::as_str) else {
                    continue;
                };
                if !crate::cmd::doctor::is_auto_command(command) {
                    continue;
                }
                let Some(next) = migrated_line(command) else {
                    continue;
                };
                jsonio::map_mut(hook).insert("command".into(), json!(next));
            }
        }
    }
}

fn migrated_line(command: &str) -> Option<String> {
    let wanted = crate::cmd::doctor::hook_wanted(command);
    let exe = canonical_exe(&exe_of(command));
    let midtask = command.split_whitespace().any(|word| {
        word.eq_ignore_ascii_case("-midtask") || word.eq_ignore_ascii_case("--midtask")
    });
    let next = line(&exe, &wanted, midtask);
    (next != command).then_some(next)
}

fn strip(settings: &mut Value) -> Vec<String> {
    let mut removed = strip_event(settings, SESSION_START);
    removed.extend(strip_event(settings, PRE_TOOL_USE));
    removed
}

fn strip_event(settings: &mut Value, event: &str) -> Vec<String> {
    let mut removed = Vec::new();
    let Some(hooks) = settings.get_mut("hooks").filter(|h| h.is_object()) else {
        return removed;
    };
    let Some(groups) = hooks.get_mut(event).and_then(Value::as_array_mut) else {
        return removed;
    };
    for group in groups.iter_mut() {
        let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        list.retain(|hook| {
            let Some(command) = hook.get("command").and_then(Value::as_str) else {
                return true;
            };
            if crate::cmd::doctor::is_auto_command(command) {
                removed.push(command.to_string());
                return false;
            }
            true
        });
    }
    groups.retain(|group| {
        group
            .get("hooks")
            .and_then(Value::as_array)
            .is_none_or(|list| !list.is_empty())
    });
    let empty = groups.is_empty();
    let hooks_map = jsonio::map_mut(hooks);
    if empty {
        hooks_map.remove(event);
    }
    if hooks_map.is_empty() {
        jsonio::map_mut(settings).remove("hooks");
    }
    removed
}

fn add(settings: &mut Value, event: &str, matcher: Option<&str>, command: &str, timeout: u64) {
    let hook = json!({ "type": "command", "command": command, "timeout": timeout });
    let mut group = json!({ "hooks": [hook] });
    if let Some(matcher) = matcher {
        jsonio::map_mut(&mut group).insert("matcher".into(), json!(matcher));
    }
    let hooks = jsonio::map_mut(settings)
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let groups = jsonio::map_mut(hooks)
        .entry(event)
        .or_insert_with(|| json!([]));
    if !groups.is_array() {
        *groups = json!([]);
    }
    if let Some(groups) = groups.as_array_mut() {
        groups.push(group);
    }
}

fn exe_of(command: &str) -> String {
    crate::cmd::doctor::quoted_words(command)
        .into_iter()
        .next()
        .unwrap_or_else(|| installed().to_string_lossy().to_string())
}

fn line(exe: &str, scope: &Wanted, midtask: bool) -> String {
    let tail = if midtask { " -Midtask" } else { "" };
    format!("{} auto{} -Hook{tail}", quoted(exe), scope.flag_clause())
}

fn canonical_exe(exe: &str) -> String {
    let text = exe.trim_matches('"').replace('\\', "/");
    let stem = text.rsplit('/').next().unwrap_or(&text);
    let stem = stem
        .strip_suffix(".exe")
        .or_else(|| stem.strip_suffix(".EXE"))
        .unwrap_or(stem)
        .to_ascii_lowercase();
    if matches!(
        stem.as_str(),
        "kebacc-codex" | "kebacc-antigravity" | "kebacc-switch"
    ) {
        let name = if cfg!(windows) {
            "kebacc.exe"
        } else {
            "kebacc"
        };
        if let Some(slash) = text.rfind('/') {
            return format!("{}{name}", &text[..=slash]);
        }
        return name.into();
    }
    text
}

fn installed() -> PathBuf {
    let name = if cfg!(windows) {
        "kebacc.exe"
    } else {
        "kebacc"
    };
    let tools = provider::home().join(".claude-tools").join(name);
    std::env::current_exe().unwrap_or(tools)
}

fn quoted(path: &str) -> String {
    let text = path.trim_matches('"').replace('\\', "/");
    format!("\"{text}\"")
}

#[cfg(test)]
mod tests {
    use super::{
        add, canonical_exe, exe_of, line, migrated_line, strip, PRE_TOOL_USE, SESSION_START,
    };
    use crate::provider::{ProviderId, Wanted};
    use serde_json::json;

    fn armed(command: &str) -> serde_json::Value {
        json!({
            "model": "opus",
            "hooks": {
                "SessionStart": [{ "hooks": [{ "type": "command", "command": command }] }]
            }
        })
    }

    #[test]
    fn disarming_leaves_no_empty_scaffolding() {
        let mut settings = armed("kebacc-switch auto -Provider all -Hook");
        let removed = strip(&mut settings);
        assert_eq!(removed.len(), 1);
        assert_eq!(settings, json!({ "model": "opus" }));
    }

    #[test]
    fn disarming_takes_the_mid_task_hook_too() {
        let mut settings = json!({
            "hooks": {
                "SessionStart": [{ "hooks": [
                    { "type": "command", "command": "kebacc auto -Provider claude -Hook" }
                ] }],
                "PreToolUse": [{ "matcher": "*", "hooks": [
                    { "type": "command", "command": "kebacc auto -Provider claude -Hook -Midtask" }
                ] }]
            }
        });
        let removed = strip(&mut settings);
        assert_eq!(removed.len(), 2);
        assert_eq!(settings, json!({}));
    }

    #[test]
    fn another_session_start_hook_is_left_alone() {
        let mut settings = json!({
            "hooks": { "SessionStart": [{ "hooks": [
                { "type": "command", "command": "kebacc auto -Provider all -Hook" },
                { "type": "command", "command": "echo hello" }
            ] }] }
        });
        strip(&mut settings);
        let list = settings["hooks"]["SessionStart"][0]["hooks"]
            .as_array()
            .expect("the other hook");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0]["command"], "echo hello");
    }

    #[test]
    fn somebody_elses_pre_tool_use_hook_survives() {
        let mut settings = json!({
            "hooks": { "PreToolUse": [{ "matcher": "Bash", "hooks": [
                { "type": "command", "command": "my-linter" }
            ] }] }
        });
        assert!(strip(&mut settings).is_empty());
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "Bash");
    }

    #[test]
    fn arming_again_only_changes_the_pool_not_the_path() {
        let command = "\"C:/Program Files/tools/kebacc.exe\" auto -Hook";
        let mut settings = armed(command);
        let removed = strip(&mut settings);
        let exe = exe_of(&removed[0]);
        let next = line(&exe, &Wanted::one(ProviderId::Claude), false);
        assert_eq!(
            next,
            "\"C:/Program Files/tools/kebacc.exe\" auto -claude -Hook"
        );
        add(&mut settings, SESSION_START, None, &next, 25);
        assert_eq!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            next
        );
    }

    #[test]
    fn the_mid_task_line_carries_the_flag_and_the_matcher() {
        let command = line("kebacc", &Wanted::one(ProviderId::Claude), true);
        assert_eq!(command, "\"kebacc\" auto -claude -Hook -Midtask");
        let mut settings = json!({});
        add(&mut settings, PRE_TOOL_USE, Some("*"), &command, 10);
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "*");
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            command
        );
    }

    #[test]
    fn a_windows_path_reaches_the_shell_with_its_separators() {
        let raw = "C:\\Users\\me\\.claude-tools\\kebacc.exe";
        let claude = Wanted::one(ProviderId::Claude);
        assert_eq!(
            line(raw, &claude, false),
            "\"C:/Users/me/.claude-tools/kebacc.exe\" auto -claude -Hook"
        );
        let old = format!("{raw} auto -Provider claude -Hook");
        assert_eq!(
            line(&exe_of(&old), &claude, false),
            "\"C:/Users/me/.claude-tools/kebacc.exe\" auto -claude -Hook"
        );
    }

    #[test]
    fn merging_two_pools_keeps_both() {
        let merged = Wanted::one(ProviderId::Codex).union(&Wanted::one(ProviderId::Claude));
        assert_eq!(merged.display(), "claude+codex");
        assert_eq!(
            Wanted::all()
                .union(&Wanted::one(ProviderId::Claude))
                .display(),
            "all"
        );
    }

    #[test]
    fn dropping_one_pool_leaves_the_rest() {
        let left = Wanted::all().minus(&Wanted::one(ProviderId::Antigravity));
        assert_eq!(
            left.ids(),
            ProviderId::ALL
                .into_iter()
                .filter(|id| *id != ProviderId::Antigravity)
                .collect::<Vec<_>>()
        );
        assert!(Wanted::one(ProviderId::Claude)
            .minus(&Wanted::one(ProviderId::Claude))
            .is_off());
    }

    #[test]
    fn an_old_provider_hook_is_rewritten() {
        let next = migrated_line("\"/tmp/kebacc\" auto -Provider antigravity -Hook").unwrap();
        assert_eq!(next, "\"/tmp/kebacc\" auto -ag -Hook");
        let leftover =
            migrated_line("\"/tmp/kebacc-codex\" auto -Provider codex -Hook -Midtask").unwrap();
        assert!(leftover.contains("auto -codex -Hook -Midtask"));
        assert!(leftover.contains("/kebacc\"") || leftover.contains("/kebacc.exe\""));
        assert!(!leftover.contains("kebacc-codex"));
    }

    #[test]
    fn leftover_binaries_are_renamed_to_kebacc() {
        let unix = canonical_exe("/tmp/kebacc-antigravity");
        assert!(unix.ends_with("/kebacc") || unix.ends_with("/kebacc.exe"));
        assert!(!unix.contains("antigravity"));
    }

    #[test]
    fn settings_without_hooks_survive_a_strip() {
        let mut settings = json!({ "model": "opus" });
        assert!(strip(&mut settings).is_empty());
        assert_eq!(settings, json!({ "model": "opus" }));
    }
}

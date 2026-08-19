use crate::jsonio;
use crate::provider;
use crate::term::{say, Color};
use serde_json::{json, Value};
use std::path::PathBuf;

const SETTINGS: [&str; 2] = ["settings.json", "settings.local.json"];
const SESSION_START: &str = "SessionStart";
const PRE_TOOL_USE: &str = "PreToolUse";
const TIMEOUT: u64 = 25;
/// The mid-task hook runs before every single tool call, so it gets a short
/// leash. All it does is read a stamp file and, at most once an interval, spawn
/// a detached `auto`: the switch itself never happens on this thread.
const MIDTASK_TIMEOUT: u64 = 10;

/// What arming does to the scope that is already in the settings.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Write the pool asked for, whatever was armed before.
    Set,
    /// Add this pool to what is armed, leaving the rest of the scope alone.
    /// Only hooks running this binary are ever read or rewritten, so a switcher
    /// installed beside this one keeps its own pair whatever this does.
    Merge,
    /// Take this pool out. This build carries one pool, so what is left is
    /// nothing it could be armed on: the hooks go. The other half's hooks run
    /// its own binary under its own name and were never touched.
    Drop,
}

/// Arm or disarm the auto-switch. This only edits the hooks in `settings.json`:
/// it never switches the account, whatever the quota says.
///
/// Two hooks go in, not one. `SessionStart` opens the next session on an
/// account with room; `PreToolUse` keeps that true *during* a run, so a quota
/// that dies mid-task is noticed then instead of at the next launch.
pub fn run(scope: &str, quiet: bool, mode: Mode) -> i32 {
    let scope = scope.trim().to_lowercase();
    let wanted = match scope.as_str() {
        "off" | "none" | "no" => None,
        "claude" | "all" => Some("claude".to_string()),
        other => {
            say(
                &format!("'{other}' is not a pool. Use claude or off."),
                Color::Red,
            );
            return 64;
        }
    };

    if wanted.is_none() && mode != Mode::Set {
        say(
            "-Merge and -Drop need a pool to add or take out, not 'off'.",
            Color::Red,
        );
        return 64;
    }

    let dir = provider::claude_config_dir();
    let mut touched = false;
    // What was actually written, which under -Merge can be wider than what was
    // asked for and under -Drop is nothing at all. Reported instead of the
    // request.
    let mut written = wanted.clone();

    for name in SETTINGS {
        let path = dir.join(name);
        let Some(mut settings) = jsonio::read(&path) else {
            continue;
        };
        let before = settings.clone();
        let existing = strip(&mut settings);
        if let (Some(asked), Some(command)) = (&wanted, existing.first()) {
            // Keep the binary the hooks already pointed at, whatever it is.
            let exe = exe_of(command);
            let scope = match mode {
                Mode::Set => Some(asked.clone()),
                Mode::Merge => Some(widen(
                    crate::cmd::doctor::hook_scope(command).as_deref(),
                    asked,
                )),
                Mode::Drop => None,
            };
            match scope {
                Some(scope) => {
                    arm_both(&mut settings, &exe, &scope);
                    written = Some(scope);
                }
                None => written = None,
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

    // Nothing was armed anywhere, so there is nothing to widen: Set and Merge
    // write the first pair of hooks, Drop has nothing to take out.
    if let Some(scope) = &wanted {
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

    if !quiet {
        match &written {
            Some(scope) => println!("auto {scope}"),
            None => println!("auto off"),
        }
    }
    0
}

/// The scope one hook has to carry to cover both what is armed and what is being
/// added. This build only ever finds its own hooks, so in practice that is
/// `claude`; `all` is still understood, and so is the other half's name, because
/// a hook written back when one binary carried both pools says one of those and
/// has to keep meaning "this pool too" rather than being replaced outright.
fn widen(existing: Option<&str>, adding: &str) -> String {
    let Some(had) = existing.map(|s| s.trim().to_lowercase()) else {
        return adding.to_string();
    };
    if had.is_empty() || had == adding {
        return adding.to_string();
    }
    match (had.as_str(), adding) {
        ("claude" | "codex" | "all", "claude" | "all") => "all".to_string(),
        // A scope this build has never heard of is not something to widen, so
        // the pool asked for takes its place.
        _ => adding.to_string(),
    }
}

/// Writes the pair: the session-start hook and the mid-task one, same binary,
/// same pool.
fn arm_both(settings: &mut Value, exe: &str, scope: &str) {
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

/// Takes every auto hook out of the settings, session-start and mid-task both,
/// and hands back their commands.
fn strip(settings: &mut Value) -> Vec<String> {
    let mut removed = strip_event(settings, SESSION_START);
    removed.extend(strip_event(settings, PRE_TOOL_USE));
    removed
}

/// Strips the auto hooks out of one event, leaving no empty group, no empty
/// event, no empty `hooks` behind.
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

/// Keeps the binary the hook already pointed at, so re-arming only changes the
/// pool and never the path.
fn exe_of(command: &str) -> String {
    crate::cmd::doctor::quoted_words(command)
        .into_iter()
        .next()
        .unwrap_or_else(|| installed().to_string_lossy().to_string())
}

fn line(exe: &str, scope: &str, midtask: bool) -> String {
    let tail = if midtask { " -Midtask" } else { "" };
    format!("{} auto -Provider {scope} -Hook{tail}", quoted(exe))
}

/// The binary the hooks should name when there is none already there to copy.
///
/// The copy running this, first: `arm` is what the installers call, they call it
/// as the binary they just installed, and an installer pointed at a tools
/// directory of its own has to arm that one and not whatever is in the default
/// place. `~/.claude-tools` is the fallback for the case where the running path
/// cannot be read at all.
fn installed() -> PathBuf {
    let name = if cfg!(windows) {
        "kebacc-switch.exe"
    } else {
        "kebacc-switch"
    };
    let tools = provider::home().join(".claude-tools").join(name);
    std::env::current_exe().unwrap_or(tools)
}

fn quoted(path: &str) -> String {
    if path.starts_with('"') || !path.contains(' ') {
        path.to_string()
    } else {
        format!("\"{path}\"")
    }
}

#[cfg(test)]
mod tests {
    use super::{add, exe_of, line, strip, widen, PRE_TOOL_USE, SESSION_START};
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
                    { "type": "command", "command": "kebacc-switch auto -Provider claude -Hook" }
                ] }],
                "PreToolUse": [{ "matcher": "*", "hooks": [
                    { "type": "command", "command": "kebacc-switch auto -Provider claude -Hook -Midtask" }
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
                { "type": "command", "command": "kebacc-switch auto -Provider all -Hook" },
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
        let command = "\"C:/Program Files/tools/kebacc-switch.exe\" auto -Provider all -Hook";
        let mut settings = armed(command);
        let removed = strip(&mut settings);
        let exe = exe_of(&removed[0]);
        let next = line(&exe, "claude", false);
        assert_eq!(
            next,
            "\"C:/Program Files/tools/kebacc-switch.exe\" auto -Provider claude -Hook"
        );
        add(&mut settings, SESSION_START, None, &next, 25);
        assert_eq!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            next
        );
    }

    #[test]
    fn the_mid_task_line_carries_the_flag_and_the_matcher() {
        let command = line("kebacc-switch", "claude", true);
        assert_eq!(
            command,
            "kebacc-switch auto -Provider claude -Hook -Midtask"
        );
        let mut settings = json!({});
        add(&mut settings, PRE_TOOL_USE, Some("*"), &command, 10);
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "*");
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            command
        );
    }

    #[test]
    fn merging_keeps_a_scope_that_covers_the_other_half_too() {
        // Written back when one binary carried both pools. Arming this one
        // again must not narrow it to this pool alone.
        assert_eq!(widen(Some("codex"), "claude"), "all");
        assert_eq!(widen(Some("all"), "claude"), "all");
    }

    #[test]
    fn merging_into_nothing_is_just_this_pool() {
        assert_eq!(widen(None, "claude"), "claude");
        assert_eq!(widen(Some(""), "claude"), "claude");
        assert_eq!(widen(Some("claude"), "claude"), "claude");
    }

    #[test]
    fn a_scope_nobody_knows_is_replaced_not_widened() {
        assert_eq!(widen(Some("sonnet"), "claude"), "claude");
    }

    #[test]
    fn settings_without_hooks_survive_a_strip() {
        let mut settings = json!({ "model": "opus" });
        assert!(strip(&mut settings).is_empty());
        assert_eq!(settings, json!({ "model": "opus" }));
    }
}

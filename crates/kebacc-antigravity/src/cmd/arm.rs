use crate::jsonio;
use crate::provider;
use crate::term::{say, Color};
use serde_json::{json, Value};
use std::path::PathBuf;

const SETTINGS: [&str; 2] = ["settings.json", "settings.local.json"];
const TIMEOUT: u64 = 25;
/// The mid-task hook runs before every tool call, so it gets a short leash. All
/// it does is read a stamp file and, at most once every few minutes, spawn a
/// detached `auto`: the switching itself never happens inside this budget.
const MIDTASK_TIMEOUT: u64 = 10;
/// The two events auto is armed on, and the flag each one's command carries.
/// `SessionStart` catches an account that was already out of quota; `PreToolUse`
/// catches one that runs out halfway through a task, which is where a long job
/// would otherwise sit on a capped account until the user noticed.
/// The one pool this build can be armed on, under the name the hooks spell it
/// with.
const PROVIDER_POOL: &str = crate::provider::PROVIDER_ID;

const EVENTS: [(&str, &str); 2] = [("SessionStart", ""), ("PreToolUse", " -Midtask")];

/// What arming does to the scope already in the settings.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Write the pool asked for, whatever was armed before.
    Set,
    /// Add this pool to what is armed, leaving the rest of the scope alone.
    /// Only hooks running this binary are ever read or rewritten, so a switcher
    /// installed beside this one keeps its own pair whatever this does.
    Merge,
    /// Take this pool out of what is armed, and disarm when that leaves
    /// nothing. Uninstalling here must not disarm anything else.
    Drop,
}

/// Arm or disarm the auto-switch. This only edits the hooks in `settings.json`:
/// it never switches the account, whatever the quota says.
pub fn run(scope: &str, quiet: bool, mode: Mode) -> i32 {
    let scope = scope.trim().to_lowercase();
    let wanted = match scope.as_str() {
        "off" | "none" | "no" => None,
        "antigravity" | "all" => Some(scope.clone()),
        other => {
            say(
                &format!("'{other}' is not a pool this build arms. Use antigravity or off."),
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
    // What was actually written, which under -Merge is wider and under -Drop
    // narrower than what was asked for. Reported instead of the request.
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
            let armed = crate::cmd::doctor::hook_scope(command);
            let scope = match mode {
                Mode::Set => Some(asked.clone()),
                Mode::Merge => Some(widen(armed.as_deref(), asked)),
                Mode::Drop => narrow(armed.as_deref(), asked),
            };
            if let Some(scope) = scope {
                for (event, flag) in EVENTS {
                    let command = command_for(&exe, &scope, flag);
                    add(&mut settings, event, &command, timeout(event));
                }
                written = Some(scope);
            } else {
                written = None;
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

    // Nothing was armed anywhere, so there is nothing to widen or narrow: Set
    // and Merge write the first pair of hooks, Drop has nothing to do.
    if let Some(scope) = &wanted {
        if !touched && mode != Mode::Drop {
            let path = dir.join(SETTINGS[0]);
            let mut settings = jsonio::read(&path).unwrap_or_else(|| json!({}));
            strip(&mut settings);
            let exe = quoted(&installed().to_string_lossy());
            for (event, flag) in EVENTS {
                let command = command_for(&exe, scope, flag);
                add(&mut settings, event, &command, timeout(event));
            }
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

/// The scope one hook has to carry to cover both what is armed and what is
/// being added. This build only ever finds its own hooks, so in practice that
/// is `antigravity`; `all` is still understood, because a hook written by an earlier
/// version — when both pools ran from one binary — says `all` and must keep
/// meaning "this pool too" rather than being read as a pool that is gone.
fn widen(existing: Option<&str>, adding: &str) -> String {
    let Some(had) = existing.map(|s| s.trim().to_lowercase()) else {
        return adding.to_string();
    };
    if had.is_empty() || had == adding {
        return adding.to_string();
    }
    match (had.as_str(), adding) {
        ("claude" | "antigravity" | "all", "claude" | "antigravity" | "all") => "all".to_string(),
        // A scope this build has never heard of is not something to widen, so
        // the pool asked for takes its place.
        _ => adding.to_string(),
    }
}

/// The scope left once one pool is taken out of it. `None` means nothing is left
/// to arm, and the caller drops the hooks entirely.
///
/// The hooks this rewrites all run this binary, and this binary carries one
/// pool. So taking that pool out leaves a scope it could not answer to: a hook
/// left behind saying `-Provider claude` runs *this* binary, which refuses that
/// name and exits non-zero — at every session start and before every tool call,
/// in front of the user. The other half's hooks run its own binary under its own
/// name and were never touched here, so dropping ours takes nothing from it.
fn narrow(existing: Option<&str>, removing: &str) -> Option<String> {
    let had = existing.map(|scope| scope.trim().to_lowercase())?;
    let pools: Vec<&str> = if had == "all" {
        vec!["claude", PROVIDER_POOL]
    } else {
        had.split('+').collect()
    };
    let left: Vec<&str> = pools
        .into_iter()
        .map(str::trim)
        .filter(|pool| !pool.is_empty() && *pool != removing)
        .collect();
    // Whatever is left, this build can only be armed on its own pool.
    if !left.contains(&PROVIDER_POOL) {
        return None;
    }
    Some(PROVIDER_POOL.to_string())
}

/// Takes every auto hook out of the settings, on every event it is armed on,
/// and hands back their commands: no empty group, no empty event, no empty
/// `hooks` left behind.
fn strip(settings: &mut Value) -> Vec<String> {
    let mut removed = Vec::new();
    for (event, _) in EVENTS {
        strip_event(settings, event, &mut removed);
    }
    if settings
        .get("hooks")
        .and_then(Value::as_object)
        .is_some_and(serde_json::Map::is_empty)
    {
        jsonio::map_mut(settings).remove("hooks");
    }
    removed
}

fn strip_event(settings: &mut Value, event: &str, removed: &mut Vec<String>) {
    let Some(hooks) = settings.get_mut("hooks").filter(|h| h.is_object()) else {
        return;
    };
    let Some(groups) = hooks.get_mut(event).and_then(Value::as_array_mut) else {
        return;
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
}

fn timeout(event: &str) -> u64 {
    if event == "SessionStart" {
        TIMEOUT
    } else {
        MIDTASK_TIMEOUT
    }
}

fn add(settings: &mut Value, event: &str, command: &str, timeout: u64) {
    let hook = json!({ "type": "command", "command": command, "timeout": timeout });
    let hooks = jsonio::map_mut(settings)
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let groups = jsonio::map_mut(hooks)
        .entry(event.to_string())
        .or_insert_with(|| json!([]));
    if !groups.is_array() {
        *groups = json!([]);
    }
    let Some(groups) = groups.as_array_mut() else {
        return;
    };
    // `SessionStart` takes no matcher. `PreToolUse` does, and it has to be every
    // tool: a task made of nothing but edits would otherwise never check the
    // quota until it was over.
    if event == "SessionStart" {
        groups.push(json!({ "hooks": [hook] }));
    } else {
        groups.push(json!({ "matcher": "*", "hooks": [hook] }));
    }
}

/// The binary a hook already pointed at, quoted as it will go back in. Arming a
/// different pool must not move the install out from under the hook.
fn exe_of(command: &str) -> String {
    let exe = crate::cmd::doctor::quoted_words(command)
        .into_iter()
        .next()
        .unwrap_or_else(|| installed().to_string_lossy().to_string());
    quoted(&exe)
}

fn command_for(exe: &str, scope: &str, flag: &str) -> String {
    format!("{exe} auto -Provider {scope} -Hook{flag}")
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
        "kebacc-antigravity.exe"
    } else {
        "kebacc-antigravity"
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
    use super::{add, command_for, exe_of, narrow, strip, widen, EVENTS};
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
        let mut settings = armed("kebacc-antigravity auto -Provider all -Hook");
        let removed = strip(&mut settings);
        assert_eq!(removed.len(), 1);
        assert_eq!(settings, json!({ "model": "opus" }));
    }

    #[test]
    fn another_session_start_hook_is_left_alone() {
        let mut settings = json!({
            "hooks": { "SessionStart": [{ "hooks": [
                { "type": "command", "command": "kebacc-antigravity auto -Provider all -Hook" },
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
    fn arming_again_only_changes_the_pool_not_the_path() {
        let command = "\"C:/Program Files/tools/kebacc-antigravity.exe\" auto -Provider all -Hook";
        let mut settings = armed(command);
        let removed = strip(&mut settings);
        let next = command_for(&exe_of(&removed[0]), "claude", "");
        assert_eq!(
            next,
            "\"C:/Program Files/tools/kebacc-antigravity.exe\" auto -Provider claude -Hook"
        );
        add(&mut settings, "SessionStart", &next, 25);
        assert_eq!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            next
        );
    }

    #[test]
    fn arming_covers_the_tool_calls_as_well_as_the_session() {
        let mut settings = json!({});
        for (event, flag) in EVENTS {
            let command = command_for("kebacc-antigravity", "claude", flag);
            add(&mut settings, event, &command, 10);
        }
        assert_eq!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            "kebacc-antigravity auto -Provider claude -Hook"
        );
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "kebacc-antigravity auto -Provider claude -Hook -Midtask"
        );
        // Without a matcher the mid-task hook would not run on every tool call.
        assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "*");
    }

    #[test]
    fn disarming_takes_the_mid_task_hook_too() {
        let mut settings = json!({
            "hooks": {
                "SessionStart": [{ "hooks": [
                    { "type": "command", "command": "kebacc-antigravity auto -Provider all -Hook" }
                ] }],
                "PreToolUse": [{ "matcher": "*", "hooks": [
                    { "type": "command", "command": "kebacc-antigravity auto -Provider all -Hook -Midtask" }
                ] }]
            }
        });
        assert_eq!(strip(&mut settings).len(), 2);
        assert_eq!(settings, json!({}));
    }

    #[test]
    fn merging_the_other_pool_into_an_armed_one_covers_both() {
        assert_eq!(widen(Some("claude"), "antigravity"), "all");
        assert_eq!(widen(Some("antigravity"), "claude"), "all");
        assert_eq!(widen(Some("all"), "antigravity"), "all");
    }

    #[test]
    fn merging_a_pool_that_is_already_armed_changes_nothing() {
        assert_eq!(widen(Some("antigravity"), "antigravity"), "antigravity");
        assert_eq!(widen(None, "antigravity"), "antigravity");
        assert_eq!(widen(Some(""), "claude"), "claude");
    }

    // shared.ps1 carries the same algebra for the installers to use, and CI
    // runs this table against it. A scope nothing here knows — `off`, or one a
    // later version writes — is replaced by the pool being armed rather than
    // joined to it, since joining would arm a hook on a pool that has no
    // meaning.
    #[test]
    fn merging_into_a_scope_this_build_does_not_know_replaces_it() {
        assert_eq!(widen(Some("off"), "claude"), "claude");
        assert_eq!(widen(Some("sonnet"), "claude"), "claude");
    }

    #[test]
    fn dropping_this_pool_out_of_a_wider_scope_disarms() {
        // The hooks run this binary, and this binary has no claude pool: left
        // armed on that name it would fail in front of the user every time.
        assert_eq!(narrow(Some("all"), "antigravity"), None);
        assert_eq!(narrow(Some("claude+antigravity"), "antigravity"), None);
    }

    #[test]
    fn dropping_a_pool_that_is_not_ours_leaves_ours_armed() {
        assert_eq!(
            narrow(Some("all"), "claude"),
            Some("antigravity".to_string())
        );
        assert_eq!(
            narrow(Some("claude+antigravity"), "claude"),
            Some("antigravity".to_string())
        );
    }

    #[test]
    fn dropping_the_only_pool_armed_disarms() {
        assert_eq!(narrow(Some("antigravity"), "antigravity"), None);
        assert_eq!(narrow(None, "antigravity"), None);
    }

    #[test]
    fn settings_without_hooks_survive_a_strip() {
        let mut settings = json!({ "model": "opus" });
        assert!(strip(&mut settings).is_empty());
        assert_eq!(settings, json!({ "model": "opus" }));
    }
}

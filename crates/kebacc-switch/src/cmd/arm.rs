use crate::jsonio;
use crate::provider;
use crate::term::{say, Color};
use serde_json::{json, Value};
use std::path::PathBuf;

const SETTINGS: [&str; 2] = ["settings.json", "settings.local.json"];
const TIMEOUT: u64 = 25;

/// Arm or disarm the session-start auto-switch. This only edits the hook in
/// `settings.json`: it never switches the account, whatever the quota says.
pub fn run(scope: &str, quiet: bool) -> i32 {
    let scope = scope.trim().to_lowercase();
    let wanted = match scope.as_str() {
        "off" | "none" | "no" => None,
        "claude" | "codex" | "all" => Some(scope.clone()),
        other => {
            say(
                &format!("'{other}' is not a pool. Use claude, codex, all or off."),
                Color::Red,
            );
            return 64;
        }
    };

    let dir = provider::claude_config_dir();
    let mut touched = false;

    for name in SETTINGS {
        let path = dir.join(name);
        let Some(mut settings) = jsonio::read(&path) else {
            continue;
        };
        let before = settings.clone();
        let existing = strip(&mut settings);
        if let Some(scope) = &wanted {
            if let Some(command) = existing.first() {
                add(&mut settings, &rewritten(command, scope), TIMEOUT);
                touched = true;
            }
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

    if let Some(scope) = &wanted {
        if !touched {
            let path = dir.join(SETTINGS[0]);
            let mut settings = jsonio::read(&path).unwrap_or_else(|| json!({}));
            strip(&mut settings);
            add(&mut settings, &fresh_command(scope), TIMEOUT);
            if let Err(problem) = jsonio::write(&path, &settings) {
                say(
                    &format!("Could not write {}: {problem}", path.display()),
                    Color::Red,
                );
                return 1;
            }
        }
    }

    if !quiet {
        match &wanted {
            Some(scope) => println!("auto {scope}"),
            None => println!("auto off"),
        }
    }
    0
}

/// Takes every auto hook out of the settings and hands back their commands,
/// leaving no empty group, no empty `SessionStart`, no empty `hooks` behind.
fn strip(settings: &mut Value) -> Vec<String> {
    let mut removed = Vec::new();
    let Some(hooks) = settings.get_mut("hooks").filter(|h| h.is_object()) else {
        return removed;
    };
    let Some(groups) = hooks.get_mut("SessionStart").and_then(Value::as_array_mut) else {
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
        hooks_map.remove("SessionStart");
    }
    if hooks_map.is_empty() {
        jsonio::map_mut(settings).remove("hooks");
    }
    removed
}

fn add(settings: &mut Value, command: &str, timeout: u64) {
    let hook = json!({ "type": "command", "command": command, "timeout": timeout });
    let hooks = jsonio::map_mut(settings)
        .entry("hooks")
        .or_insert_with(|| json!({}));
    let groups = jsonio::map_mut(hooks)
        .entry("SessionStart")
        .or_insert_with(|| json!([]));
    if !groups.is_array() {
        *groups = json!([]);
    }
    if let Some(groups) = groups.as_array_mut() {
        groups.push(json!({ "hooks": [hook] }));
    }
}

/// Keeps the binary the hook already pointed at, and only changes the pool.
fn rewritten(command: &str, scope: &str) -> String {
    let exe = crate::cmd::doctor::quoted_words(command)
        .into_iter()
        .next()
        .unwrap_or_else(|| installed().to_string_lossy().to_string());
    format!("{} auto -Provider {scope} -Hook", quoted(&exe))
}

fn fresh_command(scope: &str) -> String {
    format!(
        "{} auto -Provider {scope} -Hook",
        quoted(&installed().to_string_lossy())
    )
}

fn installed() -> PathBuf {
    let name = if cfg!(windows) {
        "kebacc-switch.exe"
    } else {
        "kebacc-switch"
    };
    let tools = provider::home().join(".claude-tools").join(name);
    if tools.exists() {
        return tools;
    }
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
    use super::{add, rewritten, strip};
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
    fn arming_again_only_changes_the_pool_not_the_path() {
        let command = "\"C:/Program Files/tools/kebacc-switch.exe\" auto -Provider all -Hook";
        let mut settings = armed(command);
        let removed = strip(&mut settings);
        let next = rewritten(&removed[0], "claude");
        assert_eq!(
            next,
            "\"C:/Program Files/tools/kebacc-switch.exe\" auto -Provider claude -Hook"
        );
        add(&mut settings, &next, 25);
        assert_eq!(
            settings["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            next
        );
    }

    #[test]
    fn settings_without_hooks_survive_a_strip() {
        let mut settings = json!({ "model": "opus" });
        assert!(strip(&mut settings).is_empty());
        assert_eq!(settings, json!({ "model": "opus" }));
    }
}

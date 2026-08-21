use crate::jsonio;
use crate::provider;
use crate::term::{say, Color};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn run(statusline: Option<bool>, updates: Option<bool>, quiet: bool) -> i32 {
    if statusline.is_none() && updates.is_none() {
        say(
            "Nothing to wire. Use -StatusLine, -NoStatusLine, -AutoUpdate or -NoAutoUpdate.",
            Color::Red,
        );
        return 64;
    }

    let Some(command) = statusline_command() else {
        say(
            "Cannot find my own path, so nothing was written.",
            Color::Red,
        );
        return 1;
    };

    let dir = provider::claude_config_dir();
    if !dir.exists() && std::fs::create_dir_all(&dir).is_err() {
        say(&format!("Could not create {}.", dir.display()), Color::Red);
        return 1;
    }
    let path = dir.join("settings.json");

    let mut settings = jsonio::read(&path).unwrap_or_else(|| json!({}));
    if !settings.is_object() {
        say(
            &format!("{} is not a JSON object. Nothing changed.", path.display()),
            Color::Red,
        );
        return 1;
    }

    let mut stash = Stash::Nothing;
    if let Some(on) = statusline {
        stash = set_statusline(&mut settings, on, &command, read_stash());
    }
    if let Some(on) = updates {
        set_updates(&mut settings, on);
    }

    keep_a_copy(&path);
    if let Err(problem) = jsonio::write(&path, &settings) {
        say(
            &format!("Could not write {}: {problem}", path.display()),
            Color::Red,
        );
        return 1;
    }

    match &stash {
        Stash::Keep(line) => write_stash(line),
        Stash::Drop => drop_stash(),
        Stash::Nothing => {}
    }

    if !quiet {
        match statusline {
            Some(true) => println!("status line on"),
            Some(false) => println!("status line off"),
            None => {}
        }
        match &stash {
            Stash::Keep(_) => {
                println!("the status line that was there is kept, and comes back on uninstall")
            }
            Stash::Drop => println!("the status line that was there before is back"),
            Stash::Nothing => {}
        }
        match updates {
            Some(true) => println!("auto update on"),
            Some(false) => println!("auto update off"),
            None => {}
        }
    }
    0
}

fn statusline_command() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let text = exe
        .display()
        .to_string()
        .replace(std::path::MAIN_SEPARATOR, "/");
    Some(format!("\"{text}\" statusline"))
}

fn keep_a_copy(path: &Path) {
    if !path.exists() {
        return;
    }
    let first = path.with_extension("json.cc-backup");
    if !first.exists() {
        let _ = std::fs::copy(path, &first);
    }
    let _ = std::fs::copy(path, path.with_extension("json.cc-backup.prev"));
}

/// There is one status line in settings.json and every tool that wants it wants
/// the same slot, so taking it means putting back whatever was there once this
/// half leaves. This is what the caller has to do with the copy it keeps.
enum Stash {
    /// Leave the copy on disk as it is.
    Nothing,
    /// Save this status line: ours displaced it.
    Keep(Value),
    /// Ours is gone, the copy has been put back, and it is no longer needed.
    Drop,
}

fn is_ours(line: Option<&Value>) -> bool {
    line.and_then(|line| line.get("command"))
        .and_then(Value::as_str)
        .is_some_and(super::doctor::is_ours_binary)
}

fn set_statusline(settings: &mut Value, on: bool, command: &str, kept: Option<Value>) -> Stash {
    let map = jsonio::map_mut(settings);
    if on {
        // Ours already there: whatever it displaced the first time is still the
        // one to put back, so the copy stays as it is.
        let displaced = match map.get("statusLine") {
            Some(line) if !is_ours(Some(line)) => Stash::Keep(line.clone()),
            _ => Stash::Nothing,
        };
        map.insert(
            "statusLine".to_string(),
            json!({ "type": "command", "command": command }),
        );
        return displaced;
    }
    if !is_ours(map.get("statusLine")) {
        // Someone else holds the slot now. Theirs stays, and so does the copy:
        // it is not ours to put back over a line we never took.
        return Stash::Nothing;
    }
    match kept {
        Some(line) => {
            map.insert("statusLine".to_string(), line);
        }
        None => {
            map.remove("statusLine");
        }
    }
    Stash::Drop
}

fn stash_path() -> PathBuf {
    provider::state_dir().join("displaced-statusline.json")
}

fn read_stash() -> Option<Value> {
    let line = jsonio::read(&stash_path())?;
    line.is_object().then_some(line)
}

fn write_stash(line: &Value) {
    let path = stash_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = jsonio::write(&path, line);
}

fn drop_stash() {
    let _ = std::fs::remove_file(stash_path());
}

fn set_updates(settings: &mut Value, on: bool) {
    let map = jsonio::map_mut(settings);
    if on {
        if let Some(block) = map.get_mut("env").and_then(Value::as_object_mut) {
            block.remove("KEBACC_SWITCH_UPDATE");
            if block.is_empty() {
                map.remove("env");
            }
        }
        return;
    }
    if !map.get("env").map(Value::is_object).unwrap_or(false) {
        map.insert("env".to_string(), json!({}));
    }
    if let Some(block) = map.get_mut("env").and_then(Value::as_object_mut) {
        block.insert("KEBACC_SWITCH_UPDATE".to_string(), json!("off"));
    }
}

#[cfg(test)]
mod tests {
    use super::{set_statusline, set_updates, Stash};
    use serde_json::json;

    const OURS: &str = "\"/tmp/kebacc\" statusline";

    #[test]
    fn the_status_line_keeps_the_rest_of_the_settings() {
        let mut settings = json!({ "model": "opus", "statusLine": { "command": "other" } });
        set_statusline(&mut settings, true, OURS, None);
        assert_eq!(settings["model"], "opus");
        assert_eq!(settings["statusLine"]["command"], OURS);
    }

    #[test]
    fn a_status_line_that_is_not_ours_is_left_alone() {
        let mut settings = json!({ "statusLine": { "command": "starship prompt" } });
        set_statusline(&mut settings, false, "x", None);
        assert_eq!(settings["statusLine"]["command"], "starship prompt");
    }

    #[test]
    fn the_status_line_we_displace_is_handed_back_to_be_kept() {
        let mut settings = json!({ "statusLine": { "command": "starship prompt" } });
        let stash = set_statusline(&mut settings, true, OURS, None);
        let Stash::Keep(line) = stash else {
            panic!("the displaced status line was not kept");
        };
        assert_eq!(line["command"], "starship prompt");
    }

    #[test]
    fn taking_ours_out_puts_the_displaced_one_back() {
        let mut settings = json!({ "statusLine": { "command": OURS } });
        let kept = json!({ "type": "command", "command": "starship prompt" });
        let stash = set_statusline(&mut settings, false, "x", Some(kept));
        assert!(matches!(stash, Stash::Drop));
        assert_eq!(settings["statusLine"]["command"], "starship prompt");
    }

    #[test]
    fn ours_going_with_nothing_kept_leaves_no_status_line() {
        let mut settings = json!({ "model": "opus", "statusLine": { "command": OURS } });
        let stash = set_statusline(&mut settings, false, "x", None);
        assert!(matches!(stash, Stash::Drop));
        assert!(settings.get("statusLine").is_none());
        assert_eq!(settings["model"], "opus");
    }

    #[test]
    fn arming_twice_does_not_keep_our_own_line_as_the_displaced_one() {
        let mut settings = json!({ "statusLine": { "command": OURS } });
        let stash = set_statusline(&mut settings, true, OURS, None);
        assert!(matches!(stash, Stash::Nothing));
    }

    #[test]
    fn a_line_taken_by_someone_else_is_not_overwritten_by_the_kept_one() {
        let mut settings = json!({ "statusLine": { "command": "starship prompt" } });
        let kept = json!({ "command": "starship prompt" });
        let stash = set_statusline(&mut settings, false, "x", Some(kept));
        assert!(matches!(stash, Stash::Nothing));
        assert_eq!(settings["statusLine"]["command"], "starship prompt");
    }

    #[test]
    fn turning_updates_off_writes_the_variable_and_on_takes_it_out() {
        let mut settings = json!({ "env": { "OTHER": "1" } });
        set_updates(&mut settings, false);
        assert_eq!(settings["env"]["KEBACC_SWITCH_UPDATE"], "off");
        set_updates(&mut settings, true);
        assert_eq!(settings["env"]["OTHER"], "1");
        assert!(settings["env"].get("KEBACC_SWITCH_UPDATE").is_none());
    }

    #[test]
    fn an_empty_env_block_does_not_survive() {
        let mut settings = json!({});
        set_updates(&mut settings, false);
        set_updates(&mut settings, true);
        assert!(settings.get("env").is_none());
    }
}

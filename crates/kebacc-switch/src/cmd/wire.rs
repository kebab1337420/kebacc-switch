use crate::jsonio;
use crate::provider;
use crate::term::{say, Color};
use serde_json::{json, Value};
use std::path::Path;

pub fn run(statusline: Option<bool>, updates: Option<bool>, quiet: bool) -> i32 {
    if statusline.is_none() && updates.is_none() {
        say(
            "Nothing to wire. Use -StatusLine, -NoStatusLine, -AutoUpdate or -NoAutoUpdate.",
            Color::Red,
        );
        return 64;
    }

    let Some(command) = statusline_command() else {
        say("Cannot find my own path, so nothing was written.", Color::Red);
        return 1;
    };

    let dir = provider::claude_config_dir();
    if !dir.exists() && std::fs::create_dir_all(&dir).is_err() {
        say(
            &format!("Could not create {}.", dir.display()),
            Color::Red,
        );
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

    if let Some(on) = statusline {
        set_statusline(&mut settings, on, &command);
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

    if !quiet {
        match statusline {
            Some(true) => println!("status line on"),
            Some(false) => println!("status line off"),
            None => {}
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
    let text = exe.display().to_string().replace(std::path::MAIN_SEPARATOR, "/");
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

fn set_statusline(settings: &mut Value, on: bool, command: &str) {
    let map = jsonio::map_mut(settings);
    if on {
        map.insert(
            "statusLine".to_string(),
            json!({ "type": "command", "command": command }),
        );
        return;
    }
    let ours = map
        .get("statusLine")
        .and_then(|line| line.get("command"))
        .and_then(Value::as_str)
        .is_some_and(|text| text.contains("kebacc-switch"));
    if ours {
        map.remove("statusLine");
    }
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
    use super::{set_statusline, set_updates};
    use serde_json::json;

    #[test]
    fn the_status_line_keeps_the_rest_of_the_settings() {
        let mut settings = json!({ "model": "opus", "statusLine": { "command": "other" } });
        set_statusline(&mut settings, true, "\"/tmp/kebacc-switch\" statusline");
        assert_eq!(settings["model"], "opus");
        assert_eq!(
            settings["statusLine"]["command"],
            "\"/tmp/kebacc-switch\" statusline"
        );
    }

    #[test]
    fn a_status_line_that_is_not_ours_is_left_alone() {
        let mut settings = json!({ "statusLine": { "command": "starship prompt" } });
        set_statusline(&mut settings, false, "x");
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

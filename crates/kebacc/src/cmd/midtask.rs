use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_INTERVAL_MS: u128 = 20 * 1000;

pub fn run(wanted: &crate::provider::Wanted) -> i32 {
    announce(super::auto::take_note());
    super::watch::ensure_running(wanted);
    if about_us(&hook_payload()) {
        return 0;
    }
    let claimed = crate::lock::locked(crate::lock::MIDTASK, || {
        let stamp = stamp_file();
        if !due(&stamp) {
            return false;
        }
        let _ = std::fs::write(&stamp, now_ms().to_string());
        true
    });
    if claimed == Ok(true) {
        spawn(wanted);
    }
    0
}

fn announce(note: Option<String>) {
    let Some(note) = note else {
        return;
    };
    let payload = serde_json::json!({ "systemMessage": note });
    println!("{payload}");
}

fn hook_payload() -> String {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return String::new();
    }
    let mut text = String::new();
    let _ = stdin.lock().read_to_string(&mut text);
    text
}

fn about_us(payload: &str) -> bool {
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(payload) else {
        return false;
    };
    let Some(command) = payload
        .get("tool_input")
        .and_then(|input| input.get("command"))
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    is_switcher(command)
}

fn is_switcher(command: &str) -> bool {
    let Some(word) = command.split_whitespace().next() else {
        return false;
    };
    let program = word
        .trim_matches(|c| c == '"' || c == '\'')
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(word)
        .to_lowercase();
    let program = program.strip_suffix(".exe").unwrap_or(&program);
    matches!(
        program,
        "kebacc" | "kebacc-switch" | "kebacc-codex" | "kebacc-antigravity"
    )
}

fn stamp_file() -> PathBuf {
    crate::provider::state_dir().join("midtask.stamp")
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub fn interval_ms() -> u128 {
    std::env::var("KEBACC_SWITCH_MIDTASK_INTERVAL_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u128>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_INTERVAL_MS)
}

fn due(stamp: &PathBuf) -> bool {
    let Some(last) = std::fs::read_to_string(stamp)
        .ok()
        .and_then(|text| text.trim().parse::<u128>().ok())
    else {
        return true;
    };
    let now = now_ms();
    last > now || now - last >= interval_ms()
}

fn spawn(wanted: &crate::provider::Wanted) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(exe);
    command.arg("auto");
    command.args(wanted.flags());
    command.args(["-Hook", "-Spawned"]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::proc::detach(&mut command);
    let _ = crate::proc::spawn_detached(&mut command);
}

#[cfg(test)]
mod tests {
    use super::{about_us, announce};

    #[test]
    fn a_switcher_command_is_left_alone() {
        assert!(about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"~/.claude-tools/kebacc list -ag"}}"#
        ));
    }

    #[test]
    fn an_unrelated_command_still_arms_the_check() {
        assert!(!about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"cargo build"}}"#
        ));
    }

    #[test]
    fn working_on_the_switcher_still_arms_the_check() {
        assert!(!about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"grep -rn kebacc crates/kebacc/src"}}"#
        ));
        assert!(!about_us(
            r#"{"tool_name":"Edit","tool_input":{"file_path":"crates/kebacc/src/cmd/midtask.rs"}}"#
        ));
    }

    #[test]
    fn the_windows_binary_counts_too() {
        assert!(about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"\"C:\\Users\\x\\.claude-tools\\kebacc.exe\" doctor"}}"#
        ));
    }

    #[test]
    fn the_other_halves_count_too() {
        assert!(about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"~/.claude-tools/kebacc-codex list"}}"#
        ));
        assert!(about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"~/.claude-tools/kebacc-antigravity list"}}"#
        ));
        assert!(about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"~/.claude-tools/kebacc-switch list"}}"#
        ));
    }

    #[test]
    fn our_own_name_outside_the_tool_input_does_not_count() {
        assert!(!about_us(
            r#"{"cwd":"C:/Users/x/dev/kebacc-switch","tool_input":{"command":"cargo test"}}"#
        ));
    }

    #[test]
    fn no_payload_at_all_arms_the_check() {
        assert!(!about_us(""));
    }

    #[test]
    fn nothing_is_said_when_no_switch_happened() {
        announce(None);
    }

    #[test]
    fn a_note_leaves_as_the_json_claude_code_reads() {
        let payload = serde_json::json!({ "systemMessage": "Switched Claude Code to a@b.c." });
        assert_eq!(payload["systemMessage"], "Switched Claude Code to a@b.c.");
    }
}

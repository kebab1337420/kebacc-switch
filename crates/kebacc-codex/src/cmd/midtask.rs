use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_INTERVAL_MS: u128 = 5 * 60 * 1000;

pub fn run(provider: &str) -> i32 {
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
        spawn(provider);
    }
    0
}

/// The `PreToolUse` payload, when Claude Code is the one calling.
fn hook_payload() -> String {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return String::new();
    }
    let mut text = String::new();
    let _ = stdin.lock().read_to_string(&mut text);
    text
}

/// A tool call that is itself a switcher command — listing the accounts,
/// checking the install — must not switch the account under the user's feet.
/// They asked to look, not to move.
fn about_us(payload: &str) -> bool {
    let payload = payload.to_lowercase();
    let Some(input) = payload.find("\"tool_input\"") else {
        return false;
    };
    payload[input..].contains("kebacc")
}

fn stamp_file() -> PathBuf {
    // The state directory is shared with the Claude switcher, and its own
    // mid-task hook reads a stamp of its own: one name each, or whichever runs
    // first silences the other for the whole interval.
    crate::provider::state_dir().join("midtask-codex.stamp")
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn interval_ms() -> u128 {
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

fn spawn(provider: &str) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(exe);
    command
        .args(["auto", "-Provider", provider, "-Hook", "-Spawned"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::proc::detach(&mut command);
    let _ = command.spawn();
}

#[cfg(test)]
mod tests {
    use super::about_us;

    #[test]
    fn a_switcher_command_is_left_alone() {
        assert!(about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"~/.claude-tools/kebacc-codex list -Provider codex"}}"#
        ));
    }

    #[test]
    fn an_unrelated_command_still_arms_the_check() {
        assert!(!about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"cargo build"}}"#
        ));
    }

    #[test]
    fn our_own_name_outside_the_tool_input_does_not_count() {
        assert!(!about_us(
            r#"{"cwd":"C:/Users/x/dev/kebacc-codex","tool_input":{"command":"cargo test"}}"#
        ));
    }

    #[test]
    fn no_payload_at_all_arms_the_check() {
        assert!(!about_us(""));
    }
}

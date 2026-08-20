use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// How often the mid-task hook is allowed to look. Five minutes used to be
/// the figure, then a minute, and both are long enough for a run of turns
/// to keep going to an account that is already refusing them. Twenty
/// seconds costs almost nothing: away from the cap the check reads the
/// cached usage and never leaves the machine.
const DEFAULT_INTERVAL_MS: u128 = 20 * 1000;

pub fn run(provider: &str) -> i32 {
    // Word of the last detached switch, if one happened since we last ran.
    // It goes out whatever else this call decides to do: the session has been
    // spending a turn on an account that moved under it, and that is worth
    // saying even on a call that is about to keep quiet.
    announce(super::auto::take_note());
    // A tool call proves the session is alive, and is the cheapest place to
    // notice the watcher died.
    super::watch::ensure_running(provider);
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

/// Claude Code reads a hook's stdout as JSON and shows `systemMessage` to the
/// user. Nothing else here can reach the session: the switch itself runs
/// detached, and its own stdout goes nowhere.
fn announce(note: Option<String>) {
    let Some(note) = note else {
        return;
    };
    let payload = serde_json::json!({ "systemMessage": note });
    println!("{payload}");
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
///
/// Only the command actually being run counts. Merely naming us — a `grep
/// kebacc`, a `cargo test` inside this repository, an edit to a file under
/// `crates/kebacc-antigravity/` — is not a switcher call, and used to cost every
/// mid-task check for the length of a session spent working on the switcher
/// itself.
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

/// The first word of a shell command, minus quotes and any directory in front
/// of it, is the program. Ours are named after the pool they carry.
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
    matches!(program, "kebacc-switch" | "kebacc-antigravity")
}

fn stamp_file() -> PathBuf {
    // The state directory is shared with the Claude switcher, and its own
    // mid-task hook reads a stamp of its own: one name each, or whichever runs
    // first silences the other for the whole interval.
    crate::provider::state_dir().join("midtask-antigravity.stamp")
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
            r#"{"tool_name":"Bash","tool_input":{"command":"~/.claude-tools/kebacc-antigravity list -Provider antigravity"}}"#
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
            r#"{"tool_name":"Bash","tool_input":{"command":"grep -rn kebacc crates/kebacc-antigravity/src"}}"#
        ));
        assert!(!about_us(
            r#"{"tool_name":"Edit","tool_input":{"file_path":"crates/kebacc-antigravity/src/cmd/midtask.rs"}}"#
        ));
    }

    #[test]
    fn the_windows_binary_counts_too() {
        assert!(about_us(
            r#"{"tool_name":"Bash","tool_input":{"command":"\"C:\\Users\\x\\.claude-tools\\kebacc-antigravity.exe\" doctor"}}"#
        ));
    }

    #[test]
    fn our_own_name_outside_the_tool_input_does_not_count() {
        assert!(!about_us(
            r#"{"cwd":"C:/Users/x/dev/kebacc-antigravity","tool_input":{"command":"cargo test"}}"#
        ));
    }

    #[test]
    fn no_payload_at_all_arms_the_check() {
        assert!(!about_us(""));
    }
}

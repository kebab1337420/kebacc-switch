//! Installing the Codex switcher, which lives on its own branch. This used to
//! be `install-codex.ps1`, downloaded from GitHub by a slash command every time
//! it was run because a copy on disk went stale.
//!
//! kebacc-switch handles Claude and nothing else. Codex has a plugin of its
//! own, built from the `Codex` branch of the same repository. It installs into
//! the same tools directory under its own name and its own version marker: the
//! two share the directory and nothing else, and each uninstaller names its own
//! files rather than sweeping.
//!
//! There is no published release for it, so this clones the branch, builds it
//! with cargo and hands the binary to whatever installer that branch carries.
//! Run it again to update. The saved logins are never touched.

use super::Options;
use crate::term::{say, Color};
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_SOURCE: &str = "https://github.com/kebab1337420/kebacc-switch.git";
const DEFAULT_BRANCH: &str = "Codex";

pub fn run(opts: &Options) -> i32 {
    let source = opts
        .source
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or(DEFAULT_SOURCE);
    let branch = opts
        .branch
        .as_deref()
        .filter(|text| !text.trim().is_empty())
        .unwrap_or(DEFAULT_BRANCH);

    for needed in ["git", "cargo"] {
        if !on_path(needed) {
            say(
                &format!(
                    "{needed} is not on the PATH, and this builds from source. Install it and run this again."
                ),
                Color::Red,
            );
            return 1;
        }
    }

    let checkout = std::env::temp_dir().join(format!("kebacc-codex-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&checkout);

    // --depth 1 on one branch: the history is not wanted, and cloning the whole
    // repository to build one crate is a minute nobody asked for.
    say(&format!("Cloning {branch} from {source}"), Color::Dim);
    let cloned = ran(Command::new("git").args([
        "clone",
        "--quiet",
        "--depth",
        "1",
        "--branch",
        branch,
        "--",
        source,
        &checkout.display().to_string(),
    ]));
    if !cloned {
        say(
            &format!("Could not clone the {branch} branch from {source}."),
            Color::Red,
        );
        say(
            "If the branch only exists locally, point at that checkout: -Source <path>",
            Color::Yellow,
        );
        return 1;
    }

    let code = build_and_install(opts, &checkout);

    if opts.keep_checkout {
        say(
            &format!("The checkout is at {}", checkout.display()),
            Color::Dim,
        );
    } else {
        let _ = std::fs::remove_dir_all(&checkout);
    }
    code
}

fn build_and_install(opts: &Options, checkout: &Path) -> i32 {
    say(
        "Building kebacc-codex, which takes a minute the first time.",
        Color::Dim,
    );
    let built = ran(Command::new("cargo").args([
        "build",
        "--release",
        "--manifest-path",
        &checkout.join("Cargo.toml").display().to_string(),
        "-p",
        "kebacc-codex",
    ]));
    if !built {
        say("The build failed. Nothing was installed.", Color::Red);
        return 1;
    }

    let binary = checkout
        .join("target")
        .join("release")
        .join(format!("kebacc-codex{}", std::env::consts::EXE_SUFFIX));
    if !binary.is_file() {
        say(
            &format!(
                "The build reported success but {} is not there.",
                binary.display()
            ),
            Color::Red,
        );
        return 1;
    }

    for mut candidate in installers(opts, checkout, &binary) {
        match ran_code(&mut candidate) {
            Some(0) => return 0,
            // 64 is what this codebase answers to a command it does not know.
            // A branch whose binary has not been converted says it here, and
            // the script it still ships is tried instead.
            Some(64) => continue,
            _ => return 1,
        }
    }
    say(
        &format!(
            "The branch at {} has nothing that installs kebacc-codex.",
            checkout.display()
        ),
        Color::Red,
    );
    1
}

/// Everything on that branch that might install it, best first.
///
/// The binary is asked to install itself before anything else: this side went
/// that way, the codex side is going the same way, and a binary that carries
/// its own installer needs no script and cannot fall out of step with one. A
/// branch whose binary does not know the command answers 64, and the caller
/// moves on to the scripts it still ships — which know which slash commands
/// are the codex ones and where its version marker goes.
fn installers(opts: &Options, checkout: &Path, binary: &Path) -> Vec<Command> {
    let mut all = Vec::new();

    let mut command = Command::new(binary);
    command.arg("install");
    if let Some(dir) = tools(opts) {
        command.arg("-ToolsDir").arg(dir);
    }
    if opts.auto_switch {
        command.arg("-AutoSwitch");
    }
    all.push(command);

    let plugin = checkout.join("plugins").join("kebacc-codex");

    let script = plugin.join("install.ps1");
    if script.is_file() {
        if let Some(shell) = powershell() {
            let mut command = Command::new(shell);
            command.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]);
            command.arg(&script);
            command.arg("-Binary").arg(binary);
            if let Some(dir) = tools(opts) {
                command.arg("-ToolsDir").arg(dir);
            }
            if opts.auto_switch {
                command.arg("-AutoSwitch");
            }
            all.push(command);
        }
    }

    let script = plugin.join("install.sh");
    if script.is_file() && on_path("sh") {
        let mut command = Command::new("sh");
        command.arg(&script);
        command.arg("--binary").arg(binary);
        if let Some(dir) = tools(opts) {
            command.arg("--tools-dir").arg(dir);
        }
        if opts.auto_switch {
            command.arg("--auto-switch");
        }
        all.push(command);
    }

    all
}

/// Left unset on purpose when nobody asked for one: the branch's own installer
/// picks the tools directory, and only an explicit value overrides it.
fn tools(opts: &Options) -> Option<PathBuf> {
    opts.tools_dir
        .as_deref()
        .filter(|dir| !dir.trim().is_empty())
        .map(PathBuf::from)
}

/// pwsh where it exists, the Windows-only `powershell` where it does not. A
/// machine with neither runs the shell script instead.
fn powershell() -> Option<&'static str> {
    ["pwsh", "powershell"]
        .into_iter()
        .find(|shell| on_path(shell))
}

fn on_path(program: &str) -> bool {
    let mut probe = Command::new(program);
    probe.arg("--version");
    crate::proc::hidden(&mut probe);
    probe
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}

fn ran(command: &mut Command) -> bool {
    ran_code(command) == Some(0)
}

/// The exit code, or `None` when the command could not be started at all — the
/// two are different here: 64 sends the caller to the next installer, and a
/// missing program does not.
fn ran_code(command: &mut Command) -> Option<i32> {
    crate::proc::hidden(command);
    command.status().ok().and_then(|status| status.code())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_that_is_not_there_is_not_on_the_path() {
        assert!(!on_path("kebacc-no-such-program-anywhere"));
    }

    #[test]
    fn the_binary_is_asked_before_any_script_on_the_branch() {
        let opts = Options::default();
        let all = installers(
            &opts,
            Path::new("/nowhere"),
            Path::new("/nowhere/kebacc-codex"),
        );
        assert!(!all.is_empty());
        assert!(all[0]
            .get_program()
            .to_string_lossy()
            .contains("kebacc-codex"));
        let args: Vec<_> = all[0].get_args().map(|arg| arg.to_string_lossy()).collect();
        assert_eq!(args, vec!["install"]);
    }

    #[test]
    fn the_tools_directory_is_only_passed_on_when_asked_for() {
        let mut opts = Options::default();
        assert!(tools(&opts).is_none());
        opts.tools_dir = Some("   ".into());
        assert!(tools(&opts).is_none());
        opts.tools_dir = Some("/tools".into());
        assert_eq!(tools(&opts), Some(PathBuf::from("/tools")));
    }
}

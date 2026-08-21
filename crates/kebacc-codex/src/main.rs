mod cmd;
mod jsonio;
mod live;
mod lock;
mod pool;
mod proc {
    pub use kebacc_core::proc::*;
}
mod provider;
mod seal {
    pub use kebacc_core::seal::*;
}
mod term {
    pub use kebacc_core::term::*;
}
mod usage;

use cmd::Options;
use term::{say, Color};

/// Keychain / libsecret account this build stores the seal key under.
/// Saved logins on existing machines only open if this stays this string.
const SEAL_ACCOUNT: &str = "kebacc-switch";

fn init() {
    kebacc_core::seal::set_secret_account(SEAL_ACCOUNT);
}

fn main() {
    init();
    cmd::update::sweep();
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&args));
}

fn usage_text() {
    println!("kebacc-codex <command> [options]");
    println!();
    println!("  add         save the login the CLI is using right now");
    println!(
        "  list        the saved logins and what is known of their quota (-Refresh to ask the API)"
    );
    println!("  switch      change which saved login the CLI uses");
    println!("  remove      forget a saved login");
    println!("  auto        switch only if the one in use is out of quota");
    println!(
        "  arm         arm or disarm the session-start auto-switch, without switching anything now"
    );
    println!("  doctor      check the install and the pool (-Protect, -Adopt, -Clean to repair, -Rollback to undo a switch)");
    println!("  statusline  the Claude Code status line, from a payload on stdin");
    println!("  update      install the newest release (-Check to only say whether one is out)");
    println!("  install     put this binary, its slash commands and its settings in place");
    println!("  uninstall   take all of that back (-Pool also deletes the saved logins)");
    println!();
    println!("  list -Countdown   both quota windows of every saved account, with their resets (-Refresh reads them again first)");
    println!("  auto -Midtask     auto from a tool-use hook, at most once an interval");
    println!("  watch             keep checking on a clock of its own, for the stretches with no tool call (the hooks start this)");
    println!("  refresh           read every saved account's quota again, silently (the status line spawns this)");
    println!("  arm -Provider codex|off   arm the session-start auto-switch, or turn it off");
    println!("  arm -Provider codex -Merge          add this pool to whatever is already armed, rather than replacing it");
    println!(
        "  arm -Provider codex -Drop           take this pool out, leaving anything else armed"
    );
    println!();
    println!("  install -StatusLine        also point the Claude Code status line at the switcher");
    println!("  install -AutoSwitch        also run auto at session start and during a task");
    println!("  install -ToolsDir <dir>    install somewhere other than ~/.claude-tools");
    println!();
    println!("  Updates install themselves once a day at session start. KEBACC_SWITCH_UPDATE=off stops that.");
}

fn dispatch(args: &[String]) -> i32 {
    let command = args
        .first()
        .map(|c| c.to_lowercase())
        .unwrap_or_else(|| "list".into());
    if matches!(command.as_str(), "-h" | "--help" | "help" | "") {
        usage_text();
        return 0;
    }
    if matches!(command.as_str(), "-v" | "--version" | "version") {
        println!("kebacc-codex {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    let command = match command.as_str() {
        "ls" => "list",
        "select" | "use" => "switch",
        "rm" => "remove",
        "save" => "add",
        "check" => "doctor",
        "upgrade" | "selfupdate" => "update",
        other => other,
    };
    if !matches!(
        command,
        "add"
            | "list"
            | "switch"
            | "remove"
            | "auto"
            | "doctor"
            | "statusline"
            | "update"
            | "refresh"
            | "arm"
            | "gitstat"
            | "wire"
            | "watch"
            | "install"
            | "uninstall"
            | "reap"
    ) {
        say(&format!("Unknown command '{command}'."), Color::Red);
        usage_text();
        return 64;
    }

    if command == "statusline" {
        return cmd::statusline::run();
    }

    if command == "gitstat" {
        let Some(root) = args.get(1) else {
            return 64;
        };
        return cmd::statusline::gitstat(std::path::Path::new(root));
    }

    let (wanted, mut options) = match parse(&args[1..]) {
        Ok(parsed) => parsed,
        Err(problem) => {
            say(&problem, Color::Red);
            return 64;
        }
    };

    if command == "wire" {
        return cmd::wire::run(options.statusline, options.updates, options.quiet);
    }

    match command {
        "install" => return cmd::install::run(&options),
        "uninstall" => return cmd::uninstall::run(&options),
        // Not in the help: it is the copy an uninstall leaves behind, waiting
        // for the name of a binary that cannot delete itself on Windows.
        "reap" => return cmd::uninstall::reap(&options),
        _ => {}
    }

    if command == "arm" {
        let mode = match (options.merge, options.drop) {
            (true, true) => {
                say("-Merge and -Drop ask for opposite things.", Color::Red);
                return 64;
            }
            (true, false) => cmd::arm::Mode::Merge,
            (false, true) => cmd::arm::Mode::Drop,
            (false, false) => cmd::arm::Mode::Set,
        };
        return cmd::arm::run(&wanted, options.quiet, mode);
    }

    if command == "refresh" {
        options.quiet = true;
    }

    if command == "update" {
        return cmd::update::run(&options);
    }

    if command == "auto" && options.hook && !options.spawned {
        cmd::update::maybe();
        // Session start is the other place a session announces itself, and the
        // one that gets the watcher up before the first tool call.
        cmd::watch::ensure_running(&wanted);
    }

    if command == "watch" {
        return cmd::watch::run(&wanted);
    }

    if command == "auto" && options.midtask {
        return cmd::midtask::run(&wanted);
    }

    match provider::resolve(&wanted) {
        Ok(()) => hushed(run(command, &options), &options),
        Err(problem) => {
            say(&problem, Color::Red);
            64
        }
    }
}

fn hushed(code: i32, options: &Options) -> i32 {
    if options.hook {
        0
    } else {
        code
    }
}

fn run(command: &str, options: &Options) -> i32 {
    let provider = provider::spec();
    match command {
        "add" => cmd::add::run(&provider, options),
        "list" if options.countdown => cmd::countdown::run(&provider, options),
        "list" => cmd::list::run(&provider, options),
        "switch" => cmd::switch::run(&provider, options),
        "remove" => cmd::remove::run(&provider, options),
        "auto" => cmd::auto::run(&provider, options),
        "refresh" => cmd::refresh::run(&provider, options),
        _ => cmd::doctor::run(&provider, options),
    }
}

fn parse(tokens: &[String]) -> Result<(String, Options), String> {
    let mut provider = provider::PROVIDER_ID.to_string();
    let mut options = Options::default();
    let mut index = 0;

    while index < tokens.len() {
        let token = &tokens[index];
        index += 1;
        let Some(name) = token.strip_prefix('-') else {
            return Err(format!(
                "Unexpected argument '{token}'. Options are named: -Email you@example.com"
            ));
        };
        let name = name
            .trim_start_matches('-')
            .replace(['-', '_'], "")
            .to_lowercase();
        let mut value = || {
            let next = tokens.get(index).filter(|t| !t.starts_with('-'));
            if next.is_some() {
                index += 1;
            }
            next.cloned()
        };
        match name.as_str() {
            "provider" | "p" => provider = value().ok_or("-Provider needs a name: codex.")?,
            "email" | "e" => options.email = Some(value().ok_or("-Email needs an address.")?),
            "quiet" => options.quiet = true,
            "hook" => {
                options.quiet = true;
                options.hook = true;
            }
            "refresh" => options.refresh = true,
            "yes" | "y" => options.yes = true,
            "protect" => options.protect = true,
            "adopt" => options.adopt = true,
            "rollback" => options.rollback = true,
            "clean" => options.clean = true,
            "countdown" => options.countdown = true,
            "midtask" => options.midtask = true,
            "merge" => options.merge = true,
            "drop" => options.drop = true,
            "check" => options.check = true,
            "spawned" => options.spawned = true,
            "statusline" => options.statusline = Some(true),
            "nostatusline" => options.statusline = Some(false),
            "autoupdate" => options.updates = Some(true),
            "noautoupdate" => options.updates = Some(false),
            "toolsdir" => options.tools_dir = Some(value().ok_or("-ToolsDir needs a path.")?),
            "binary" => options.binary = Some(value().ok_or("-Binary needs a path.")?),
            // A switch here, where the Claude half takes a pool name: this
            // binary carries one pool, and it is the only one it could arm.
            "autoswitch" => options.auto_switch = true,
            "noprofileedit" => options.no_profile_edit = true,
            "pool" => options.pool = true,
            other => return Err(format!("Unknown option '-{other}'.")),
        }
    }
    Ok((provider, options))
}

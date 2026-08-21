mod cmd;
mod jsonio;
mod live;
mod lock;
mod log;
mod oauth;
mod pool;
mod proc;
mod provider;
mod seal;
mod term;
mod usage;

use cmd::Options;
use provider::ProviderId;
use term::{say, Color};

fn main() {
    cmd::update::sweep();
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&args));
}

fn usage_text() {
    println!("kebacc <command> [options]");
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
    println!("  doctor      check the install and the pool (-Protect, -Adopt, -Clean, -Renew to repair, -Rollback to undo a switch)");
    println!("  statusline  the Claude Code status line, from a payload on stdin");
    println!("  update      install the newest release (-Check to only say whether one is out)");
    println!("  install     put this binary, the slash commands and the hooks in place (-StatusLine, -AutoSwitch, -ToolsDir, -NoProfileEdit)");
    println!(
        "  uninstall   take all of that back, leaving the saved logins (-Pool removes those too)"
    );
    println!("  install-codex   clone, build and install the Codex half from its own branch");
    println!();
    println!("  list -Countdown   both quota windows of every saved account, with their resets (-Refresh reads them again first)");
    println!("  auto -Midtask     auto from a tool-use hook, at most once an interval");
    println!("  watch             keep checking on a clock of its own, for the stretches with no tool call (the hooks start this)");
    println!("  refresh           read every saved account's quota again, silently (the status line spawns this)");
    println!("  arm -Provider claude|off   arm the session-start auto-switch, or turn it off");
    println!("  arm -Provider claude -Merge         add this pool to whatever is already armed, rather than replacing it");
    println!(
        "  arm -Provider claude -Drop          take this pool out, leaving anything else armed"
    );
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
        println!("kebacc {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    let command = match command.as_str() {
        "ls" => "list",
        "select" | "use" => "switch",
        "rm" => "remove",
        "save" => "add",
        "check" => "doctor",
        "upgrade" | "selfupdate" => "update",
        "installcodex" | "codex" => "install-codex",
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
            | "install-codex"
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

    match command {
        "install" => return cmd::install::run(&options),
        "uninstall" => return cmd::uninstall::run(&options),
        "install-codex" => return cmd::codex::run(&options),
        // Not in the help: it is the copy an uninstall leaves behind to take
        // the name of the binary once this process lets go of it.
        "reap" => return cmd::uninstall::reap(&options),
        _ => {}
    }

    if command == "wire" {
        return cmd::wire::run(options.statusline, options.updates, options.quiet);
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

    // `all` is kept as a spelling of `claude`: the hooks written before Codex
    // moved out to its own plugin still say `-Provider all`.
    if provider::is_all(&wanted) {
        return hushed(run(command, ProviderId::Claude, &options), &options);
    }

    match provider::resolve(&wanted) {
        Ok(id) => hushed(run(command, id, &options), &options),
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

fn run(command: &str, id: ProviderId, options: &Options) -> i32 {
    let provider = provider::spec(id);
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
    let mut provider = "claude".to_string();
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
        // `-ToolsDir` and `--tools-dir` are the same option: the two installers
        // this replaced took the Windows spelling and the POSIX one, and the
        // instructions people have already been given use both.
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
            "provider" | "p" => provider = value().ok_or("-Provider needs a name: claude.")?,
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
            "renew" => options.renew = true,
            "clean" => options.clean = true,
            "countdown" => options.countdown = true,
            "midtask" => options.midtask = true,
            "merge" => options.merge = true,
            "drop" => options.drop = true,
            "check" => options.check = true,
            "spawned" => options.spawned = true,
            "statusline" => options.statusline = Some(true),
            "nostatusline" => options.statusline = Some(false),
            "toolsdir" => options.tools_dir = Some(value().ok_or("-ToolsDir needs a directory.")?),
            "binary" => options.binary = Some(value().ok_or("-Binary needs a path.")?),
            // `-AutoSwitch claude` is accepted so the words the old installers
            // took still work; claude is the only pool this plugin has.
            "autoswitch" => {
                let _ = value();
                options.auto_switch = true;
            }
            "noprofileedit" => options.no_profile_edit = true,
            "pool" => options.pool = true,
            "source" => options.source = Some(value().ok_or("-Source needs a URL or a path.")?),
            "branch" => options.branch = Some(value().ok_or("-Branch needs a name.")?),
            "keepcheckout" => options.keep_checkout = true,
            "autoupdate" => options.updates = Some(true),
            "noautoupdate" => options.updates = Some(false),
            other => return Err(format!("Unknown option '-{other}'.")),
        }
    }
    Ok((provider, options))
}

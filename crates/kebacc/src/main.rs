mod cmd;
mod jsonio;
mod keyring;
mod live;
mod lock;
mod log;
mod oauth;
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
use provider::{parse_pool_name, ProviderId};
use term::{say, Color};

fn bind_seal(id: ProviderId) {
    kebacc_core::seal::set_secret_account(id.seal_account());
}

fn main() {
    cmd::update::sweep();
    cmd::arm::migrate();
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&args));
}

fn usage_text() {
    println!("kebacc <command> [-claude|-cl] [-codex|-cx] [-antigravity|-ag] [-all]");
    println!();
    println!("  list        saved logins and their quota  (-Refresh asks the API)");
    println!("  add         save the login the CLI is using right now");
    println!("  switch      put a saved login in front");
    println!("  remove      forget a saved login");
    println!("  auto        switch only if the one in use is capped");
    println!("  arm         turn the auto-switch on or off, change nothing now");
    println!("  doctor      check the install and the pools (-Protect, -Adopt, -Clean, -Renew to repair, -Rollback to undo a switch)");
    println!("  statusline  the Claude Code status line, from a payload on stdin");
    println!("  update      install the newest release (-Check to only say whether one is out)");
    println!("  install     put the binary, slash commands and hooks in place");
    println!("  uninstall   take that back; saved logins stay (-Pool removes those too)");
    println!();
    println!("  kebacc list -ag");
    println!("  kebacc list -claude -ag");
    println!("  kebacc add -ag");
    println!("  kebacc switch -codex -Email you@example.com");
    println!("  kebacc arm -ag");
    println!("  kebacc arm off");
    println!();
    println!("  list, auto, doctor, arm: no flag means every pool");
    println!("  add, switch, remove: name one pool");
    println!("  arm -claude -Merge    add Claude to whatever is already armed");
    println!("  arm -ag -Drop         take Antigravity out, leave the rest");
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
        bind_seal(ProviderId::Claude);
        return cmd::statusline::run();
    }

    if command == "gitstat" {
        let Some(root) = args.get(1) else {
            return 64;
        };
        return cmd::statusline::gitstat(std::path::Path::new(root));
    }

    let mut options = match parse(&args[1..]) {
        Ok(parsed) => parsed,
        Err(problem) => {
            say(&problem, Color::Red);
            return 64;
        }
    };

    match command {
        "install" => return cmd::install::run(&options),
        "uninstall" => return cmd::uninstall::run(&options),
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
        return cmd::arm::run(&options.wanted, options.quiet, mode);
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
        cmd::watch::ensure_running(&options.wanted);
    }

    if command == "watch" {
        return cmd::watch::run(&options.wanted);
    }

    if command == "auto" && options.midtask {
        return cmd::midtask::run(&options.wanted);
    }

    if matches!(command, "add" | "switch" | "remove") {
        let Some(id) = options.wanted.exactly_one() else {
            say("Name a pool: -claude, -codex or -ag.", Color::Red);
            return 64;
        };
        bind_seal(id);
        return hushed(run(command, id, &options), &options);
    }

    let mut code = 0;
    for id in options.wanted.ids() {
        bind_seal(id);
        let next = run(command, id, &options);
        if code == 0 {
            code = next;
        }
    }
    hushed(code, &options)
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

fn parse(tokens: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;

    while index < tokens.len() {
        let token = &tokens[index];
        index += 1;
        if !token.starts_with('-') {
            if token.contains('@') {
                options.email = Some(token.clone());
                continue;
            }
            match parse_pool_name(token) {
                Some(name) => options.wanted.apply_name(name),
                None => {
                    return Err(format!(
                        "Unexpected argument '{token}'. Pools are -claude, -codex, -ag."
                    ))
                }
            }
            continue;
        }
        // `-ToolsDir` and `--tools-dir` are the same option: the two installers
        // this replaced took the Windows spelling and the POSIX one, and the
        // instructions people have already been given use both.
        let name = token
            .trim_start_matches('-')
            .replace(['-', '_'], "")
            .to_lowercase();
        if let Some(pool) = parse_pool_name(&name) {
            if !matches!(name.as_str(), "provider" | "p") {
                options.wanted.apply_name(pool);
                continue;
            }
        }
        let mut value = || {
            let next = tokens.get(index).filter(|t| !t.starts_with('-'));
            if next.is_some() {
                index += 1;
            }
            next.cloned()
        };
        match name.as_str() {
            "provider" | "p" => {
                let given = value().ok_or("Name a pool: -claude, -codex or -ag.")?;
                match parse_pool_name(&given) {
                    Some(pool) => options.wanted.apply_name(pool),
                    None => {
                        return Err(format!(
                            "Unknown pool '{given}'. Use -claude, -codex, -ag or -all."
                        ))
                    }
                }
            }
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
            "autoswitch" => options.auto_switch = true,
            "noprofileedit" => options.no_profile_edit = true,
            "pool" => options.pool = true,
            "autoupdate" => options.updates = Some(true),
            "noautoupdate" => options.updates = Some(false),
            other => return Err(format!("Unknown option '-{other}'.")),
        }
    }
    Ok(options)
}

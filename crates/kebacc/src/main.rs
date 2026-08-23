mod branch;
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

pub fn bind_seal(id: ProviderId) {
    kebacc_core::seal::set_secret_account(id.seal_account());
}

fn main() {
    cmd::update::sweep();
    cmd::arm::migrate();
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&args));
}

fn usage_text() {
    println!("kebacc <command> [-claude|-cc] [-codex|-cx] [-antigravity|-ag] [-all]");
    println!();
    println!("  status      what is live, what it has left, what is armed");
    println!(
        "  list        saved logins and their quota  (-Refresh asks the API, -Json for a script)"
    );
    println!("  add         save the login the CLI is using right now");
    println!("  switch      put a saved login in front");
    println!("  remove      forget a saved login");
    println!("  auto        switch only if the one in use is capped");
    println!("  arm         turn the auto-switch on or off, change nothing now");
    println!("  set         per-pool settings (-Rank <n>, -Reserve, -FiveHour <pct>, -SevenDay <pct>, -OnSwitch <cmd>)");
    println!("  doctor      check the install and the pools (-Fix repairs everything it can; -Protect, -Adopt, -Clean, -Renew one at a time, -Rollback to undo a switch)");
    println!("  use         set a session directory up on one account, for a terminal of its own");
    println!("  watch       the background switcher: 'watch status', 'watch stop'");
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
    println!("  kebacc set -cc -Rank 10 -Email you@example.com");
    println!("  kebacc set -cc -FiveHour 90");
    println!("  kebacc set -cc -Reserve -Email spare@example.com");
    println!("  kebacc use -cc -Email other@example.com");
    println!();
    println!("  list, auto, doctor, arm: no flag means every pool");
    println!("  add, switch, remove, set, use: name one pool");
    println!("  arm -claude -Merge    add Claude to whatever is already armed");
    println!("  arm -ag -Drop         take Antigravity out, leave the rest");
    println!();
    println!("  Updates install themselves once a day at session start. KEBACC_SWITCH_UPDATE=off stops that.");
}

fn dispatch(args: &[String]) -> i32 {
    let command = args.first().map(|c| c.to_lowercase()).unwrap_or_default();
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
            | "set"
            | "status"
            | "use"
    ) {
        say(&format!("Unknown command '{command}'."), Color::Red);
        usage_text();
        return 64;
    }

    if command == "statusline" {
        bind_seal(ProviderId::Claude);
        let claude = provider::spec(ProviderId::Claude);
        usage::use_pool_caps(pool::Pool::new(&claude).caps());
        return cmd::statusline::run();
    }

    if command == "gitstat" {
        let Some(root) = args.get(1) else {
            return 64;
        };
        return cmd::statusline::gitstat(std::path::Path::new(root));
    }

    if command == "watch" {
        match args.get(1).map(|a| a.to_lowercase()).as_deref() {
            Some("stop") => {
                let stopped = cmd::watch::stop_and_wait(std::time::Duration::from_secs(5));
                say(
                    if stopped {
                        "The watcher is down."
                    } else {
                        "No watcher was running."
                    },
                    Color::Dim,
                );
                return 0;
            }
            Some("status") => {
                let up = cmd::watch::on_duty();
                println!(
                    "watcher {} (every {}s)",
                    if up { "up" } else { "down" },
                    cmd::watch::interval().as_secs()
                );
                return i32::from(!up);
            }
            _ => {}
        }
    }

    let mut options = match parse(args.get(1..).unwrap_or(&[])) {
        Ok(parsed) => parsed,
        Err(problem) => {
            say(&problem, Color::Red);
            return 64;
        }
    };

    match command {
        "install" => return cmd::install::run(&options),
        "uninstall" => return cmd::uninstall::run(&options),
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

    if waits_on_the_terminal(command, &options) {
        cmd::update::maybe();
        cmd::watch::ensure_running(&options.wanted);
        options.offline = true;
    }

    if command == "watch" {
        return cmd::watch::run(&options.wanted);
    }

    if command == "status" {
        return cmd::status::run(&options.wanted, &options);
    }

    if command == "auto" && options.midtask {
        return cmd::midtask::run(&options.wanted);
    }

    if matches!(command, "add" | "switch" | "remove" | "set" | "use") {
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
    usage::use_pool_caps(pool::Pool::new(&provider).caps());
    match command {
        "add" => cmd::add::run(&provider, options),
        "list" if options.countdown => cmd::countdown::run(&provider, options),
        "list" => cmd::list::run(&provider, options),
        "switch" => cmd::switch::run(&provider, options),
        "remove" => cmd::remove::run(&provider, options),
        "set" => cmd::set::run(&provider, options),
        "use" => cmd::use_dir::run(&provider, options),
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
            "offline" => options.offline = true,
            "statusline" => options.statusline = Some(true),
            "nostatusline" => options.statusline = Some(false),
            "toolsdir" => options.tools_dir = Some(value().ok_or("-ToolsDir needs a directory.")?),
            "dir" => options.dir = Some(value().ok_or("-Dir needs a directory.")?),
            "binary" => options.binary = Some(value().ok_or("-Binary needs a path.")?),
            "autoswitch" => options.auto_switch = true,
            "noprofileedit" => options.no_profile_edit = true,
            "pool" => options.pool = true,
            "json" => options.json = true,
            "fix" => {
                options.fix = true;
                options.protect = true;
                options.adopt = true;
                options.clean = true;
            }
            "reserve" => options.reserve = Some(true),
            "noreserve" => options.reserve = Some(false),
            "onswitch" => {
                options.on_switch =
                    Some(value().ok_or("-OnSwitch needs a command, or '' to clear it.")?)
            }
            "rank" => {
                let given = value().ok_or("-Rank needs a number.")?;
                options.rank = Some(given.trim().parse().map_err(|_| "-Rank needs a number.")?);
            }
            "fivehour" | "5h" => options.five_hour = Some(cap_value(value(), "-FiveHour")?),
            "sevenday" | "7d" => options.seven_day = Some(cap_value(value(), "-SevenDay")?),
            "autoupdate" => options.updates = Some(true),
            "noautoupdate" => options.updates = Some(false),
            other => return Err(format!("Unknown option '-{other}'.")),
        }
    }
    Ok(options)
}

fn cap_value(given: Option<String>, flag: &str) -> Result<f64, String> {
    let given = given.ok_or(format!("{flag} needs a percentage, or 'off'."))?;
    if given.trim().eq_ignore_ascii_case("off") {
        return Ok(0.0);
    }
    given
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| *value > 0.0 && *value <= 100.0)
        .ok_or(format!("{flag} takes 1 to 100, or 'off'."))
}

fn waits_on_the_terminal(command: &str, options: &Options) -> bool {
    command == "auto" && options.hook && !options.spawned && !options.midtask
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_is_help() {
        assert_eq!(dispatch(&[]), 0);
    }

    #[test]
    fn the_session_start_hook_is_the_one_the_terminal_waits_on() {
        let hook = parse(&["-Hook".to_string()]).expect("parses");
        assert!(waits_on_the_terminal("auto", &hook));
    }

    #[test]
    fn the_hooks_nobody_waits_on_still_read_the_live_numbers() {
        let midtask = parse(&["-Hook".to_string(), "-Midtask".to_string()]).expect("parses");
        assert!(!waits_on_the_terminal("auto", &midtask));
        let spawned = parse(&["-Hook".to_string(), "-Spawned".to_string()]).expect("parses");
        assert!(!waits_on_the_terminal("auto", &spawned));
        let by_hand = parse(&[]).expect("parses");
        assert!(!waits_on_the_terminal("auto", &by_hand));
    }

    #[test]
    fn help_tokens_are_help() {
        for token in ["help", "-h", "--help", ""] {
            assert_eq!(dispatch(&[token.to_string()]), 0, "{token}");
        }
    }
}

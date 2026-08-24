use super::Options;
use crate::branch::Quota;
use crate::pool::Pool;
use crate::provider::Provider;
use crate::term::{say, Color};

pub fn run(provider: &Provider, opts: &Options) -> i32 {
    let pool = Pool::new(provider);
    let mut touched = false;

    if (opts.five_hour.is_some() || opts.seven_day.is_some())
        && matches!(provider.id.branch().quota, Quota::None)
    {
        say(
            &format!(
                "{} publishes no usage, so a switching threshold would never be read.",
                provider.label
            ),
            Color::Red,
        );
        return 64;
    }

    if opts.five_hour.is_some() || opts.seven_day.is_some() {
        let current = pool.caps();
        let asked = |given: Option<f64>, kept: Option<f64>| match given {
            Some(value) if value > 0.0 => Some(value),
            Some(_) => None,
            None => kept,
        };
        let five = asked(opts.five_hour, current[0]);
        let seven = asked(opts.seven_day, current[1]);
        if !pool.set_caps(five, seven) {
            say("Could not write the pool settings.", Color::Red);
            return 1;
        }
        let text = |value: Option<f64>| match value {
            Some(value) => format!("{value}%"),
            None => "default".to_string(),
        };
        say(
            &format!(
                "{}: switch at 5h {} / 7d {}.",
                provider.label,
                text(five),
                text(seven)
            ),
            Color::Green,
        );
        touched = true;
    }

    if let Some(command) = opts.on_switch.as_deref() {
        let command = command.trim();
        let asked = (!command.is_empty()).then_some(command);
        if !pool.set_on_switch(asked) {
            say("Could not write the pool settings.", Color::Red);
            return 1;
        }
        say(
            &match asked {
                Some(command) => format!("{}: after a switch, run {command}", provider.label),
                None => format!("{}: nothing runs after a switch.", provider.label),
            },
            Color::Green,
        );
        touched = true;
    }

    if opts.rank.is_some() || opts.reserve.is_some() {
        let entries = pool.entries();
        let entry = match opts.email.as_deref() {
            Some(email) => super::switch::pick(&entries, None, Some(email)),
            None => super::current(provider, &entries),
        };
        let Some(entry) = entry else {
            say(
                "Name the account: kebacc set -Rank <n> -Email you@example.com",
                Color::Red,
            );
            return 1;
        };
        let file_name = entry.file_name();
        let missing = "That account is not in the manifest. Run: kebacc doctor -Adopt";
        if let Some(rank) = opts.rank {
            if !pool.set_priority(&file_name, rank) {
                say(missing, Color::Red);
                return 1;
            }
            say(
                &format!("{} is now rank {rank}.", entry.email),
                Color::Green,
            );
        }
        if let Some(reserve) = opts.reserve {
            if !pool.set_reserve(&file_name, reserve) {
                say(missing, Color::Red);
                return 1;
            }
            say(
                &format!(
                    "{} is {}.",
                    entry.email,
                    if reserve {
                        "held back until every other account is capped"
                    } else {
                        "back in the normal rotation"
                    }
                ),
                Color::Green,
            );
        }
        touched = true;
    }

    if !touched {
        say(
            "Nothing to set. Use -Rank <n>, -Reserve, -FiveHour <pct>, -SevenDay <pct> (or 'off') or -OnSwitch <cmd>.",
            Color::Yellow,
        );
        return 64;
    }
    0
}

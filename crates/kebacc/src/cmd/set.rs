use super::Options;
use crate::pool::Pool;
use crate::provider::Provider;
use crate::term::{say, Color};

pub fn run(provider: &Provider, opts: &Options) -> i32 {
    let pool = Pool::new(provider);
    let mut touched = false;

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

    if let Some(rank) = opts.rank {
        let entries = pool.entries();
        let Some(entry) = super::switch::pick(&entries, None, opts.email.as_deref()) else {
            say("No matching account.", Color::Red);
            return 1;
        };
        if !pool.set_priority(&entry.file_name(), rank) {
            say(
                "That account is not in the manifest. Run: kebacc doctor -Adopt",
                Color::Red,
            );
            return 1;
        }
        say(
            &format!("{} is now rank {rank}.", entry.email),
            Color::Green,
        );
        touched = true;
    }

    if !touched {
        say(
            "Nothing to set. Use -Rank <n>, -FiveHour <pct> or -SevenDay <pct> (or 'off').",
            Color::Yellow,
        );
        return 64;
    }
    0
}

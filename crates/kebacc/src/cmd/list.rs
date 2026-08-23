use super::Options;
use crate::pool::{Entry, Pool, Trust};
use crate::provider::Provider;
use crate::term::{say, Color};
use crate::usage;
use serde_json::{json, Value};

pub fn run(provider: &Provider, opts: &Options) -> i32 {
    let pool = Pool::new(provider).entries();
    if opts.json {
        return as_json(provider, opts, &pool);
    }
    say(
        &format!("{} — {}", provider.label, provider.store.display()),
        Color::Cyan,
    );
    if pool.is_empty() {
        say("No accounts saved yet.", Color::Yellow);
        say(
            &format!(
                "Log in to {}, then run: kebacc add {}",
                provider.label,
                crate::provider::Wanted::flag_of(provider.id)
            ),
            Color::Dim,
        );
        return 0;
    }

    let current = super::current(provider, &pool);
    let readings = readings(provider, opts, &pool);
    let ranked = pool.iter().any(|entry| entry.priority != 0);
    let mut broken = false;

    for (index, entry) in pool.iter().enumerate() {
        let live = current.is_some_and(|c| c.file == entry.file);
        let mark = if live { '*' } else { ' ' };
        let usage = readings.get(index).cloned().flatten();
        let pair = match &usage {
            Some(usage) => usage.as_pair(),
            None => "usage n/a".to_string(),
        };
        let mut line = format!("{mark} {:<34} {pair}", entry.email);

        if let Some(ready) = usage.as_ref().and_then(|u| u.ready_at()) {
            line.push_str(&format!("  back in {}", usage::wait_text(ready)));
        }
        if !opts.refresh {
            match usage::cache_age(entry.cache.as_ref()) {
                Some(age) => line.push_str(&format!("  ({} ago)", usage::age_text(age))),
                None if usage.is_some() => line.push_str("  (age unknown)"),
                None => {}
            }
        }
        if ranked {
            line.push_str(&format!("  rank {}", entry.priority));
        }
        if entry.reserve {
            line.push_str("  reserve");
        }

        let mut colour = if live { Color::Green } else { Color::Plain };
        if usage.as_ref().is_some_and(|u| !u.usable()) {
            colour = Color::Dim;
        }
        say(&line, colour);

        if entry.trust != Trust::Trusted {
            let (text, colour) = entry.trust.verdict();
            say(&format!("  ! {text}"), colour);
            broken |= entry.trust == Trust::Changed;
        }
        if !entry.protected {
            say("  ! stored in plain text", Color::Yellow);
        }
        if entry.creds.is_none() {
            say("  ! credentials could not be read back", Color::Red);
            broken = true;
        }
    }

    if current.is_none() {
        say("The live login is not one of the saved ones.", Color::Dim);
    }
    i32::from(broken)
}

fn readings(provider: &Provider, opts: &Options, pool: &[Entry]) -> Vec<Option<usage::Usage>> {
    if opts.refresh {
        let all: Vec<&Entry> = pool.iter().collect();
        return usage::for_entries(provider, &all, true);
    }
    pool.iter()
        .map(|entry| usage::from_cache(entry.cache.as_ref()))
        .collect()
}

fn as_json(provider: &Provider, opts: &Options, pool: &[Entry]) -> i32 {
    let current = super::current(provider, pool);
    let readings = readings(provider, opts, pool);
    let accounts: Vec<Value> = pool
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let usage = readings.get(index).cloned().flatten();
            json!({
                "email": entry.email,
                "file": entry.file,
                "live": current.is_some_and(|c| c.file == entry.file),
                "trust": entry.trust.verdict().0,
                "protected": entry.protected,
                "priority": entry.priority,
                "reserve": entry.reserve,
                "fiveHour": usage.as_ref().and_then(|u| u.pct("five_hour")),
                "sevenDay": usage.as_ref().and_then(|u| u.pct("seven_day")),
                "usable": usage.as_ref().map(usage::Usage::usable),
                "readyAt": usage
                    .as_ref()
                    .and_then(usage::Usage::ready_at)
                    .map(|at| at.to_rfc3339()),
                "checkedSecondsAgo": usage::cache_age(entry.cache.as_ref())
                    .map(|age| age.num_seconds()),
            })
        })
        .collect();
    let caps = usage::caps();
    println!(
        "{}",
        json!({
            "pool": provider.label,
            "store": provider.store,
            "caps": { "fiveHour": caps[0].1, "sevenDay": caps[1].1 },
            "accounts": accounts,
        })
    );
    0
}

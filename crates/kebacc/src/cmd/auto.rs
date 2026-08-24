use super::Options;
use crate::branch::Quota;
use crate::live;
use crate::pool::{Pool, Trust};
use crate::provider::Provider;
use crate::term::{say, Color};
use crate::usage;
use chrono::{DateTime, Utc};
use std::time::{Duration, Instant};

const BUDGET: Duration = Duration::from_secs(12);

pub fn run(provider: &Provider, opts: &Options) -> i32 {
    let note = |text: &str, colour: Color| {
        if !opts.quiet {
            say(text, colour);
        }
    };
    let started = Instant::now();
    let cached_only = || opts.offline || started.elapsed() >= BUDGET;
    let read = |entry: &crate::pool::Entry| {
        if cached_only() {
            usage::from_cache(entry.cache.as_ref())
        } else {
            usage::for_entry(provider, entry, false)
        }
    };

    if matches!(provider.id.branch().quota, Quota::None) {
        note(
            &format!(
                "{} publishes no usage, so there is nothing to switch on. Switch it by hand.",
                provider.label
            ),
            Color::Yellow,
        );
        return 30;
    }

    let pool = Pool::new(provider).entries();
    if pool.len() < 2 {
        note(
            &format!(
                "Nothing to switch between: {} has {} saved account(s).",
                provider.label,
                pool.len()
            ),
            Color::Yellow,
        );
        return 30;
    }

    let current = super::current(provider, &pool);
    if current.is_none() {
        note(
            &format!(
                "The live {} login is not one of the saved ones. It will be backed up, not kept.",
                provider.label
            ),
            Color::Yellow,
        );
    }

    let mut warning: Option<String> = None;
    let mut blind = false;
    if let Some(current) = current {
        let usage = read(current);
        let pair = usage
            .as_ref()
            .map(|u| u.as_pair())
            .unwrap_or_else(|| "usage n/a".into());
        let known = usage.as_ref().is_some_and(usage::Usage::known);
        if known && usage.as_ref().is_some_and(usage::Usage::usable) {
            note(
                &format!("{} still has room ({pair}).", current.email),
                Color::Dim,
            );
            return 0;
        }
        if known {
            note(
                &format!("{} is out of quota ({pair}).", current.email),
                Color::Yellow,
            );
            warning = Some(format!(
                "{} has reached the end of its quota ({pair}).",
                current.email
            ));
        } else {
            blind = true;
            note(
                &format!(
                    "{}: no quota reading. Looking for an account that reports room.",
                    current.email
                ),
                Color::Yellow,
            );
        }
    }

    let mut soonest: Option<DateTime<Utc>> = None;
    let mut fallback: Vec<(&crate::pool::Entry, Option<usage::Usage>)> = Vec::new();

    let mut candidates: Vec<&crate::pool::Entry> = pool
        .iter()
        .filter(|entry| !current.is_some_and(|c| c.file == entry.file))
        .filter(|entry| entry.trust != Trust::Changed && entry.creds.is_some())
        .collect();
    candidates.sort_by_key(|entry| (entry.reserve, std::cmp::Reverse(entry.priority)));
    let readings = if cached_only() {
        candidates
            .iter()
            .map(|entry| usage::from_cache(entry.cache.as_ref()))
            .collect()
    } else {
        usage::for_entries(provider, &candidates, false)
    };

    for (index, entry) in candidates.iter().enumerate() {
        let usage = readings.get(index).cloned().flatten();
        let readable = usage.as_ref().is_some_and(usage::Usage::known);
        if readable && !usage.as_ref().is_some_and(usage::Usage::usable) {
            if let Some(ready) = usage.as_ref().and_then(usage::Usage::ready_at) {
                if soonest.is_none_or(|current| ready < current) {
                    soonest = Some(ready);
                }
            }
            continue;
        }
        if !readable || entry.trust != Trust::Trusted {
            fallback.push((entry, usage));
            continue;
        }
        return take(
            provider,
            entry,
            usage.as_ref(),
            opts.spawned,
            warning.as_deref(),
            &note,
        );
    }

    if blind {
        note(
            "No account reports room either. Staying where we are rather than guessing.",
            Color::Yellow,
        );
        return 0;
    }

    if let Some((entry, usage)) = fallback.first() {
        if entry.trust != Trust::Trusted {
            note(
                &format!(
                    "{} is {}. Taking it anyway: every trusted account is out of quota.",
                    entry.email,
                    entry.trust.verdict().0
                ),
                Color::Yellow,
            );
        } else {
            note(
                &format!("{}: no quota reading, trying it anyway.", entry.email),
                Color::Dim,
            );
        }
        return take(
            provider,
            entry,
            usage.as_ref(),
            opts.spawned,
            warning.as_deref(),
            &note,
        );
    }

    let capped = match soonest {
        Some(at) => format!(
            "Every saved account is capped. The first one is back in {}.",
            usage::wait_text(at)
        ),
        None => "Every saved account is capped.".to_string(),
    };
    note(&capped, Color::Red);
    if opts.spawned {
        let head = warning.map(|text| format!("{text} ")).unwrap_or_default();
        leave_note(&format!(
            "{head}{capped} Carry on: the switch happens on its own as soon as one has room."
        ));
    }
    20
}

fn take(
    provider: &Provider,
    entry: &crate::pool::Entry,
    usage: Option<&usage::Usage>,
    spawned: bool,
    warning: Option<&str>,
    note: &dyn Fn(&str, Color),
) -> i32 {
    let activation = match live::activate(provider, entry) {
        Ok(activation) => activation,
        Err(problem) => {
            note(&problem, Color::Red);
            return 1;
        }
    };
    if let Some(text) = activation.warning.as_deref() {
        note(text, Color::Yellow);
    }
    let pair = usage
        .map(|u| u.as_pair())
        .unwrap_or_else(|| "usage n/a".into());
    let line = format!("Switched {} to {} ({pair}).", provider.label, entry.email);
    if spawned {
        let head = warning.map(|text| format!("{text} ")).unwrap_or_default();
        leave_note(&format!(
            "{head}{line} This session goes on from there: nothing to stop or wind down for."
        ));
    }
    note(&line, Color::Green);
    10
}

pub fn note_file() -> std::path::PathBuf {
    crate::provider::session_dir().join("switched.note")
}

fn leave_note(text: &str) {
    let _ = std::fs::write(note_file(), text);
}

pub fn take_note() -> Option<String> {
    let text = std::fs::read_to_string(note_file()).ok()?;
    let _ = std::fs::remove_file(note_file());
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

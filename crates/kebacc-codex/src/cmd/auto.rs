use super::Options;
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
    let read = |entry: &crate::pool::Entry| {
        if started.elapsed() >= BUDGET {
            usage::from_cache(entry.cache.as_ref())
        } else {
            usage::for_entry(provider, entry, false)
        }
    };

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

    let candidates: Vec<&crate::pool::Entry> = pool
        .iter()
        .filter(|entry| !current.is_some_and(|c| c.file == entry.file))
        .filter(|entry| entry.trust != Trust::Changed && entry.creds.is_some())
        .collect();
    let readings = if started.elapsed() >= BUDGET {
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
        return take(provider, entry, usage.as_ref(), opts.spawned, &note);
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
        return take(provider, entry, usage.as_ref(), opts.spawned, &note);
    }

    match soonest {
        Some(at) => note(
            &format!(
                "Every saved account is capped. The first one is back in {}.",
                usage::wait_text(at)
            ),
            Color::Red,
        ),
        None => note("Every saved account is capped.", Color::Red),
    }
    20
}

fn take(
    provider: &Provider,
    entry: &crate::pool::Entry,
    usage: Option<&usage::Usage>,
    spawned: bool,
    note: &dyn Fn(&str, Color),
) -> i32 {
    if let Err(problem) = live::activate(provider, entry) {
        note(&problem, Color::Red);
        return 1;
    }
    let pair = usage
        .map(|u| u.as_pair())
        .unwrap_or_else(|| "usage n/a".into());
    let line = format!("Switched {} to {} ({pair}).", provider.label, entry.email);
    // A mid-task switch runs detached, with nowhere to print: whoever spawned
    // it is long gone and the session never hears about it. Leave the line
    // where the next hook can pick it up. A switch anyone can see the output of
    // needs no note.
    if spawned {
        leave_note(&format!(
            "{line} This session goes on from here on that one."
        ));
    }
    note(&line, Color::Green);
    10
}

/// Where a detached switch leaves word of what it did. Read and removed by the
/// next mid-task hook, which is the first thing after it with a way to reach
/// the session.
///
/// Named for this pool. The state directory is the one the Claude half uses,
/// and a session with both armed would otherwise have one switch overwrite the
/// other's note and go out announcing the wrong pool.
pub fn note_file() -> std::path::PathBuf {
    crate::provider::state_dir().join("switched-codex.note")
}

fn leave_note(text: &str) {
    let _ = std::fs::write(note_file(), text);
}

/// Take the note left by a detached switch, if there is one. Reading it clears
/// it: the same switch is never announced twice.
pub fn take_note() -> Option<String> {
    let text = std::fs::read_to_string(note_file()).ok()?;
    let _ = std::fs::remove_file(note_file());
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

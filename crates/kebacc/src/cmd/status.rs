use super::Options;
use crate::pool::Pool;
use crate::provider::{self, Wanted};
use crate::term::{say, Color};
use crate::usage;

pub fn run(wanted: &Wanted, opts: &Options) -> i32 {
    say(
        &format!("kebacc {}", env!("CARGO_PKG_VERSION")),
        Color::Cyan,
    );

    for id in wanted.ids() {
        crate::bind_seal(id);
        let provider = provider::spec(id);
        let pool = Pool::new(&provider);
        usage::use_pool_caps(pool.caps());
        let entries = pool.entries();
        let current = super::current(&provider, &entries);
        let line = match current {
            Some(entry) => {
                let usage = if opts.refresh {
                    usage::for_entry(&provider, entry, true)
                } else {
                    usage::from_cache(entry.cache.as_ref())
                };
                let pair = usage
                    .as_ref()
                    .map(usage::Usage::as_pair)
                    .unwrap_or_else(|| "usage n/a".into());
                let mut text = format!("{:<12} {} — {pair}", provider.label, entry.email);
                if let Some(ready) = usage.as_ref().and_then(usage::Usage::ready_at) {
                    text.push_str(&format!("  back in {}", usage::wait_text(ready)));
                }
                if let Some(pace) = soonest_pace(&entry.snapshot) {
                    text.push_str(&format!(
                        "  {:.0}%/h, capped in {}",
                        pace.per_hour,
                        usage::wait_text(pace.full_at)
                    ));
                }
                text
            }
            None if entries.is_empty() => {
                format!("{:<12} nothing saved", provider.label)
            }
            None => format!(
                "{:<12} the live login is not one of the {} saved",
                provider.label,
                entries.len()
            ),
        };
        say(&line, Color::Plain);
    }

    let armed = super::arm::armed();
    say(
        &match &armed {
            Some(scope) => format!("auto         armed for {}", scope.display()),
            None => "auto         off".to_string(),
        },
        armed.as_ref().map_or(Color::Yellow, |_| Color::Green),
    );

    let watching = super::watch::on_duty();
    say(
        &format!(
            "watcher      {} (every {}s)",
            if watching { "up" } else { "not running" },
            super::watch::interval().as_secs()
        ),
        if watching || armed.is_none() {
            Color::Dim
        } else {
            Color::Yellow
        },
    );

    if let Some(last) = last_switch() {
        say(&format!("last switch  {last}"), Color::Dim);
    }
    0
}

fn soonest_pace(snapshot: &serde_json::Value) -> Option<usage::Pace> {
    usage::caps()
        .iter()
        .filter_map(|(name, cap)| usage::pace(snapshot, name, *cap))
        .min_by_key(|pace| pace.full_at)
}

fn last_switch() -> Option<String> {
    let text = std::fs::read_to_string(provider::session_dir().join("switch.last")).ok()?;
    let mut words = text.split_whitespace();
    let at = usage::parse_time(words.next()?)?;
    let rest: Vec<&str> = words.collect();
    Some(format!(
        "{} ago — {}",
        usage::age_text(chrono::Utc::now() - at),
        rest.join(" ")
    ))
}

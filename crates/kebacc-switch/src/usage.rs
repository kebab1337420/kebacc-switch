use crate::jsonio;
use crate::lock;
use crate::pool::Entry;
use crate::provider::Provider;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::path::Path;

pub const FIVE_HOUR_CAP: f64 = 99.0;
pub const SEVEN_DAY_CAP: f64 = 99.0;
const CACHE_SECONDS: i64 = 60;
/// How close to a cap a reading has to be for the cache to stop being trusted.
/// Far from the cap, a minute-old number cannot be wrong in a way that matters:
/// nothing burns 40 points of a window in a minute. Near it, that same minute
/// is the difference between switching in time and spending a turn on an
/// account that is already refusing.
const HOT_MARGIN: f64 = 10.0;
/// What the cache is worth once a reading is that close. Short enough that the
/// switch lands inside the margin the threshold leaves, long enough that a
/// burst of tool calls does not fetch once per call.
const HOT_CACHE_SECONDS: i64 = 5;

pub fn caps() -> [(&'static str, f64); 2] {
    [
        (
            "five_hour",
            cap_from_env("CLAUDE_AUTOSWITCH_THRESHOLD", FIVE_HOUR_CAP),
        ),
        (
            "seven_day",
            cap_from_env("CLAUDE_AUTOSWITCH_WEEKLY_THRESHOLD", SEVEN_DAY_CAP),
        ),
    ]
}

fn cap_from_env(name: &str, fallback: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.trim().parse::<f64>().ok())
        .filter(|value| *value > 0.0 && *value <= 100.0)
        .unwrap_or(fallback)
}

pub fn at_cap(pct: f64, cap: f64) -> bool {
    pct >= cap
}

pub fn debug(text: &str) {
    if std::env::var_os("KEBACC_SWITCH_DEBUG").is_none() {
        return;
    }
    use std::io::Write;
    let path = crate::provider::state_dir().join("debug.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{} {text}", now_iso());
    }
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
}

#[derive(Clone)]
pub struct Usage {
    pub five_hour: Option<Window>,
    pub seven_day: Option<Window>,
}

#[derive(Clone)]
pub struct Window {
    pub utilization: f64,
    pub resets_at: Option<String>,
}

impl Window {
    pub fn resets(&self) -> Option<DateTime<Utc>> {
        self.resets_at.as_deref().and_then(parse_time)
    }

    pub fn blocking(&self, cap: f64) -> bool {
        if !at_cap(self.utilization, cap) {
            return false;
        }
        self.resets().is_none_or(|at| at > Utc::now())
    }

    pub fn stale(&self) -> bool {
        self.resets().is_some_and(|at| at <= Utc::now())
    }
}

impl Usage {
    pub fn window(&self, name: &str) -> Option<&Window> {
        match name {
            "five_hour" => self.five_hour.as_ref(),
            _ => self.seven_day.as_ref(),
        }
    }

    pub fn pct(&self, name: &str) -> Option<f64> {
        self.window(name).map(|w| w.utilization)
    }

    pub fn known(&self) -> bool {
        self.five_hour.is_some() || self.seven_day.is_some()
    }

    pub fn usable(&self) -> bool {
        !caps()
            .iter()
            .any(|(name, cap)| self.window(name).is_some_and(|w| w.blocking(*cap)))
    }

    pub fn ready_at(&self) -> Option<DateTime<Utc>> {
        let mut at: Option<DateTime<Utc>> = None;
        for (name, cap) in caps() {
            let Some(window) = self.window(name).filter(|w| w.blocking(cap)) else {
                continue;
            };
            let Some(when) = window.resets() else {
                continue;
            };
            if at.is_none_or(|current| when > current) {
                at = Some(when);
            }
        }
        at
    }

    pub fn as_pair(&self) -> String {
        let five = self.pct("five_hour");
        let seven = self.pct("seven_day");
        if five.is_none() && seven.is_none() {
            return "usage n/a".into();
        }
        format!("5h {} / 7d {} used", pct_text(five), pct_text(seven))
    }
}

pub fn parse_time(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

pub fn pct_text(value: Option<f64>) -> String {
    let text = match value {
        None => "?".to_string(),
        Some(v) if v > 99.0 && v < 100.0 => format!("{v:.1}%"),
        Some(v) => format!("{}%", v.round() as i64),
    };
    format!("{text:>4}")
}

pub fn wait_text(at: DateTime<Utc>) -> String {
    let span = at - Utc::now();
    let seconds = span.num_seconds();
    if seconds <= 0 {
        return "now".into();
    }
    let minutes = (seconds as f64 / 60.0).ceil() as i64;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h{:02}m", minutes % 60);
    }
    format!("{}d{:02}h", hours / 24, hours % 24)
}

pub fn window_from(value: Option<&Value>) -> Option<Window> {
    let value = value.filter(|v| !v.is_null())?;
    let pct = ["used_percent", "utilization", "used_percentage"]
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_f64))?;
    let resets = match jsonio::str_of(value, "resets_at") {
        Some(at) => Some(at),
        None => value
            .get("resets_in_seconds")
            .and_then(Value::as_f64)
            .map(|secs| {
                (Utc::now() + chrono::Duration::seconds(secs as i64))
                    .to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
            }),
    };
    Some(Window {
        utilization: (pct * 10.0).round() / 10.0,
        resets_at: resets,
    })
}

pub fn window_now(value: Option<&Value>) -> Option<Window> {
    let window = window_from(value)?;
    if !window.stale() {
        return Some(window);
    }
    Some(Window {
        utilization: 0.0,
        resets_at: None,
    })
}

pub fn access_token(creds_raw: Option<&str>) -> Option<String> {
    let creds: Value = serde_json::from_str(creds_raw?).ok()?;
    let oauth = creds.get("claudeAiOauth").filter(|v| !v.is_null())?;
    jsonio::str_of(oauth, "accessToken")
}

pub fn agent() -> ureq::Agent {
    agent_with_timeout(8)
}

pub fn agent_with_timeout(seconds: u64) -> ureq::Agent {
    let config =
        ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(seconds)));
    #[cfg(windows)]
    let config = config.tls_config(
        ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build(),
    );
    config.build().new_agent()
}

fn get_json(url: &str, headers: &[(&str, &str)]) -> Option<Value> {
    let agent = agent();
    let mut request = agent.get(url);
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut response = match request.call() {
        Ok(response) => response,
        Err(problem) => {
            debug(&format!("{url} did not answer: {problem}"));
            return None;
        }
    };
    if !response.status().is_success() {
        debug(&format!("{url} answered {}", response.status()));
        return None;
    }
    match response.body_mut().read_json::<Value>() {
        Ok(value) => Some(value),
        Err(problem) => {
            debug(&format!("{url} answered something unreadable: {problem}"));
            None
        }
    }
}

pub fn fetch(token: Option<&str>) -> Option<Usage> {
    let token = token?;
    let raw = get_json(
        "https://api.anthropic.com/api/oauth/usage",
        &[
            ("Authorization", &format!("Bearer {token}")),
            ("anthropic-version", "2023-06-01"),
            ("anthropic-beta", "oauth-2025-04-20"),
        ],
    )?;
    Some(Usage {
        five_hour: window_from(raw.get("five_hour")),
        seven_day: window_from(raw.get("seven_day")),
    })
}

pub fn from_cache(cache: Option<&Value>) -> Option<Usage> {
    let cache = cache?;
    Some(Usage {
        five_hour: window_now(cache.get("five_hour")),
        seven_day: window_now(cache.get("seven_day")),
    })
}

pub fn cache_rolled_over(cache: Option<&Value>) -> bool {
    let Some(cache) = cache else {
        return false;
    };
    ["five_hour", "seven_day"]
        .iter()
        .filter_map(|name| window_from(cache.get(*name)))
        .any(|window| window.stale())
}

pub fn cache_older_than(cache: Option<&Value>, seconds: i64) -> bool {
    let Some(at) = cache
        .and_then(|c| jsonio::str_of(c, "checkedAt"))
        .and_then(|at| parse_time(&at))
    else {
        return true;
    };
    (Utc::now() - at).num_seconds() >= seconds
}

fn cache_fresh(cache: Option<&Value>) -> bool {
    let Some(at) = cache.and_then(|c| jsonio::str_of(c, "checkedAt")) else {
        return false;
    };
    let Some(at) = parse_time(&at) else {
        return false;
    };
    (Utc::now() - at).num_seconds() < cache_seconds(cache)
}

/// How long a cached reading may be trusted, given what it says. A window
/// already within `HOT_MARGIN` of its cap gets a short leash; everything else
/// keeps the full minute.
pub fn cache_seconds(cache: Option<&Value>) -> i64 {
    let hot = caps().iter().any(|(name, cap)| {
        window_now(cache.and_then(|c| c.get(*name)))
            .is_some_and(|window| window.utilization >= cap - HOT_MARGIN)
    });
    if hot {
        HOT_CACHE_SECONDS
    } else {
        CACHE_SECONDS
    }
}

pub fn save_cache(file: &Path, usage: &Usage) {
    let _ = lock::locked(lock::USAGE_CACHE, || {
        let Some(mut snapshot) = jsonio::read(file) else {
            return;
        };
        let mut cache = serde_json::Map::new();
        cache.insert("checkedAt".into(), json!(now_iso()));
        for (name, window) in [
            ("five_hour", &usage.five_hour),
            ("seven_day", &usage.seven_day),
        ] {
            if let Some(window) = window {
                cache.insert(
                    name.into(),
                    json!({ "utilization": window.utilization, "resets_at": window.resets_at }),
                );
            }
        }
        jsonio::map_mut(&mut snapshot).insert("usageCache".into(), Value::Object(cache));
        let _ = jsonio::write(file, &snapshot);
    });
}

const REFRESH_LANES: usize = 8;

pub fn for_entries(provider: &Provider, entries: &[&Entry], force: bool) -> Vec<Option<Usage>> {
    if entries.len() < 2 {
        return entries
            .iter()
            .map(|entry| for_entry(provider, entry, force))
            .collect();
    }
    let mut out = Vec::with_capacity(entries.len());
    for lane in entries.chunks(REFRESH_LANES) {
        std::thread::scope(|scope| {
            let running: Vec<_> = lane
                .iter()
                .map(|entry| scope.spawn(move || for_entry(provider, entry, force)))
                .collect();
            out.extend(running.into_iter().map(|task| task.join().unwrap_or(None)));
        });
    }
    out
}

pub fn for_entry(provider: &Provider, entry: &Entry, force: bool) -> Option<Usage> {
    let cached = from_cache(entry.cache.as_ref());
    let usable_cache =
        !force && cache_fresh(entry.cache.as_ref()) && !cache_rolled_over(entry.cache.as_ref());
    if usable_cache {
        return cached;
    }
    let live = live_token(provider, entry);
    if std::env::var_os("KEBACC_SWITCH_DEBUG").is_some() {
        debug(&format!(
            "{}: live token {}, snapshot token {}",
            entry.email,
            if live.is_some() { "yes" } else { "no" },
            if access_token(entry.creds.as_deref()).is_some() {
                "yes"
            } else {
                "no"
            }
        ));
    }
    let token = live.or_else(|| access_token(entry.creds.as_deref()));
    match fetch(token.as_deref()) {
        Some(usage) => {
            save_cache(&entry.file, &usage);
            Some(usage)
        }
        None => cached,
    }
}

fn live_token(provider: &Provider, entry: &Entry) -> Option<String> {
    let live = crate::live::identity(provider)?;
    let email = jsonio::str_of(&live, "emailAddress")?.to_lowercase();
    if email != entry.email.to_lowercase() {
        return None;
    }
    access_token(crate::live::creds_raw(provider).as_deref())
}

#[cfg(test)]
mod tests {
    use super::{cache_rolled_over, window_from, window_now, Usage};
    use serde_json::json;

    #[test]
    fn a_window_without_a_percentage_is_unknown_not_empty() {
        assert!(window_from(Some(&json!({ "resets_at": "2026-01-01T00:00:00Z" }))).is_none());
    }

    #[test]
    fn a_window_past_its_reset_reads_as_empty() {
        let closed = json!({ "utilization": 100.0, "resets_at": "2020-01-01T00:00:00Z" });
        let window = window_now(Some(&closed)).expect("a window");
        assert_eq!(window.utilization, 0.0);
        assert!(window.resets_at.is_none());
        assert!(cache_rolled_over(Some(&json!({ "five_hour": closed }))));
    }

    #[test]
    fn a_window_still_open_is_left_alone() {
        let open = json!({ "utilization": 80.0, "resets_at": "2099-01-01T00:00:00Z" });
        let window = window_now(Some(&open)).expect("a window");
        assert_eq!(window.utilization, 80.0);
        assert!(!cache_rolled_over(Some(&json!({ "five_hour": open }))));
    }

    #[test]
    fn a_window_with_a_percentage_is_read() {
        let window = window_from(Some(&json!({ "utilization": 42.25 }))).expect("a window");
        assert_eq!(window.utilization, 42.3);
    }

    #[test]
    fn a_reading_whose_window_already_reset_is_stale_not_blocking() {
        let window = super::Window {
            utilization: 100.0,
            resets_at: Some("2000-01-01T00:00:00Z".into()),
        };
        assert!(window.stale());
        assert!(!window.blocking(super::FIVE_HOUR_CAP));
    }

    #[test]
    fn a_reading_at_the_cap_with_its_reset_ahead_blocks() {
        let window = super::Window {
            utilization: 100.0,
            resets_at: Some("2099-01-01T00:00:00Z".into()),
        };
        assert!(!window.stale());
        assert!(window.blocking(super::FIVE_HOUR_CAP));
    }

    #[test]
    fn a_reading_far_from_the_cap_keeps_the_full_minute() {
        let cache = json!({
            "five_hour": { "utilization": 30.0, "resets_at": "2099-01-01T00:00:00Z" },
            "seven_day": { "utilization": 20.0, "resets_at": "2099-01-01T00:00:00Z" }
        });
        assert_eq!(super::cache_seconds(Some(&cache)), super::CACHE_SECONDS);
    }

    #[test]
    fn a_reading_near_the_cap_is_trusted_for_seconds_only() {
        let cache = json!({
            "five_hour": { "utilization": 95.0, "resets_at": "2099-01-01T00:00:00Z" },
            "seven_day": { "utilization": 20.0, "resets_at": "2099-01-01T00:00:00Z" }
        });
        assert_eq!(super::cache_seconds(Some(&cache)), super::HOT_CACHE_SECONDS);
    }

    #[test]
    fn a_window_that_already_reset_is_not_hot() {
        let cache = json!({
            "five_hour": { "utilization": 100.0, "resets_at": "2020-01-01T00:00:00Z" }
        });
        assert_eq!(super::cache_seconds(Some(&cache)), super::CACHE_SECONDS);
    }

    #[test]
    fn the_cap_is_a_plain_threshold() {
        assert!(super::at_cap(99.0, super::FIVE_HOUR_CAP));
        assert!(super::at_cap(99.4, super::FIVE_HOUR_CAP));
        assert!(!super::at_cap(98.9, super::FIVE_HOUR_CAP));
    }

    #[test]
    fn usage_with_no_window_is_not_known() {
        let usage = Usage {
            five_hour: None,
            seven_day: None,
        };
        assert!(!usage.known());
        assert!(usage.usable());
    }
}

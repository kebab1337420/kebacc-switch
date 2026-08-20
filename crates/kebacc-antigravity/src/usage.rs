use crate::jsonio;
use crate::lock;
use crate::pool::Entry;
use crate::provider::Provider;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::path::Path;

/// Where an account counts as spent. Not 100, and not 99 either: the switch has
/// to land while there is still quota left to spend, since the check runs a
/// moment before the requests it protects. Two points is the margin that buys —
/// enough for the turn in flight, small enough that almost nothing is left
/// unused on the account being left behind.
pub const FIVE_HOUR_CAP: f64 = 98.0;
pub const SEVEN_DAY_CAP: f64 = 98.0;
const CACHE_SECONDS: i64 = 60;

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
        format!("max {} / min {} used", pct_text(five), pct_text(seven))
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

/// Antigravity signs in through Google, so the client its refresh tokens are
/// minted for is Antigravity's own. It is not a secret in any useful sense: it
/// ships inside the product on every machine that installs it, and a refresh
/// token is worthless without the account it belongs to.
const OAUTH_CLIENT_ID: &str =
    "1071006060591-tmhssin2h21lcre235vtolojh4g403ep.apps.googleusercontent.com";
const OAUTH_CLIENT_SECRET: &str = "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";
const OAUTH_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Antigravity asks Google for an hour at a time, so a saved login older than
/// that carries an access token that has already lapsed. This is the margin
/// taken off the stated expiry, so a token about to lapse mid-request is
/// refreshed rather than sent.
const EXPIRY_MARGIN_SECONDS: i64 = 120;

/// The quota endpoints, in the order they are tried. The sandbox hosts answer
/// for accounts enrolled in a preview and turn everyone else away, which is why
/// a miss falls through to the next rather than ending the read.
const QUOTA_URLS: [&str; 3] = [
    "https://daily-cloudcode-pa.sandbox.googleapis.com/v1internal:fetchAvailableModels",
    "https://daily-cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
    "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels",
];
const LOAD_PROJECT_URL: &str = "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";

/// Cloud Code answers a caller it recognises. Antigravity identifies itself
/// this way, and a request that does not is turned away.
const USER_AGENT: &str = "antigravity/windows/amd64";

/// A usable access token for a saved login.
///
/// The one inside the payload is good for an hour, so most reads here find a
/// lapsed one and have to spend the refresh token on a new one. The refreshed
/// token is deliberately not written back: this process reads quota, and the
/// login on disk belongs to whichever account is signed in, not to the one
/// being asked about.
pub fn access_token(_provider: &Provider, creds_raw: Option<&str>) -> Option<String> {
    let creds: Value = serde_json::from_str(creds_raw?).ok()?;
    let token = crate::live::token_of(&creds)?;
    let live = jsonio::str_of(token, "access_token").filter(|_| !expired(token));
    if let Some(token) = live {
        return Some(token);
    }
    refreshed(&jsonio::str_of(token, "refresh_token")?)
}

/// Whether the access token in a payload has run out. A payload that states no
/// expiry is taken as run out rather than as good forever: refreshing costs one
/// request, and sending a lapsed token costs the reading entirely.
fn expired(token: &Value) -> bool {
    let stated = jsonio::str_of(token, "expiry")
        .and_then(|at| parse_time(&at))
        .or_else(|| {
            token
                .get("expiry_timestamp")
                .and_then(Value::as_f64)
                .and_then(|secs| DateTime::from_timestamp(secs as i64, 0))
        });
    let Some(at) = stated else {
        return true;
    };
    at <= Utc::now() + chrono::Duration::seconds(EXPIRY_MARGIN_SECONDS)
}

/// Spends a refresh token on a fresh access token.
fn refreshed(refresh_token: &str) -> Option<String> {
    let form = [
        ("client_id", OAUTH_CLIENT_ID),
        ("client_secret", OAUTH_CLIENT_SECRET),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    let mut response = match agent().post(OAUTH_TOKEN_URL).send_form(form) {
        Ok(response) => response,
        Err(problem) => {
            debug(&format!("the token could not be refreshed: {problem}"));
            return None;
        }
    };
    if !response.status().is_success() {
        debug(&format!(
            "the token could not be refreshed: Google answered {}",
            response.status()
        ));
        return None;
    }
    let body = response.body_mut().read_json::<Value>().ok()?;
    jsonio::str_of(&body, "access_token")
}

/// The address behind a payload, asked of Google.
///
/// This is the last resort `live::antigravity_identity` falls back to, and it
/// runs at most once per account: the address it finds is written into the
/// snapshot, and every later look-up reads it from there.
pub fn email_from_google(creds: &Value) -> Option<String> {
    let raw = serde_json::to_string(creds).ok()?;
    let token = access_token(&crate::provider::spec(), Some(&raw))?;
    let info = get_json(
        "https://www.googleapis.com/oauth2/v3/userinfo",
        &[("Authorization", &format!("Bearer {token}"))],
    )?;
    jsonio::str_of(&info, "email")
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

fn post_json(url: &str, token: &str, body: Value) -> Result<Value, u16> {
    let sent = agent()
        .post(url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("User-Agent", USER_AGENT)
        .send_json(&body);
    let mut response = match sent {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(code)) => {
            debug(&format!("{url} answered {code}"));
            return Err(code);
        }
        Err(problem) => {
            debug(&format!("{url} did not answer: {problem}"));
            return Err(0);
        }
    };
    response.body_mut().read_json::<Value>().map_err(|problem| {
        debug(&format!("{url} answered something unreadable: {problem}"));
        0
    })
}

/// The Cloud Code project this account is served under, which the quota call
/// wants alongside the token. An account without one still gets an answer, so a
/// miss here is passed on rather than treated as a failure.
fn project_of(token: &str) -> Option<String> {
    let body = post_json(
        LOAD_PROJECT_URL,
        token,
        json!({ "metadata": { "ideType": "ANTIGRAVITY" } }),
    )
    .ok()?;
    jsonio::str_of(&body, "cloudaicompanionProject")
}

/// What is left of this account's quota.
///
/// Antigravity does not meter an account the way Codex does. There are no two
/// fixed windows: there is one allowance per model family, each with a reset of
/// its own, and they empty at wildly different rates because a prompt to the
/// premium model and a prompt to the fast one cost different things. So the two
/// slots this tool carries are filled with the two readings that actually
/// decide anything: the family that is furthest gone, which is what stops work
/// first, and the family with the most left, which is what work falls back to.
pub fn fetch(_provider: &Provider, token: Option<&str>) -> Option<Usage> {
    let token = token?;
    let project = project_of(token);
    let body = match &project {
        Some(project) => json!({ "project": project }),
        None => json!({}),
    };

    let mut answer = None;
    for url in QUOTA_URLS {
        match post_json(url, token, body.clone()) {
            Ok(value) => {
                answer = Some(value);
                break;
            }
            // Anything this account is simply not entitled to is worth trying
            // the next host for. A refusal of the token itself is not: the
            // other two hosts would only repeat it.
            Err(401) | Err(403) => return None,
            Err(_) => continue,
        }
    }
    windows_from_models(answer?.get("models"))
}

/// Turns the per-model allowances into the pair of readings the rest of this
/// tool works in. `remainingFraction` is what is left, from 0 to 1, so what
/// goes in is its complement: the share already used.
fn windows_from_models(models: Option<&Value>) -> Option<Usage> {
    let models = models?.as_object()?;
    let mut readings: Vec<Window> = models
        .values()
        .filter_map(|model| model.get("quotaInfo"))
        .filter_map(|quota| {
            let remaining = quota.get("remainingFraction").and_then(Value::as_f64)?;
            Some(Window {
                utilization: (((1.0 - remaining) * 100.0) * 10.0).round() / 10.0,
                resets_at: jsonio::str_of(quota, "resetTime"),
            })
        })
        .collect();
    if readings.is_empty() {
        return None;
    }
    readings.sort_by(|left, right| {
        left.utilization
            .partial_cmp(&right.utilization)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Some(Usage {
        five_hour: readings.last().cloned(),
        seven_day: readings.first().cloned(),
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
    (Utc::now() - at).num_seconds() < CACHE_SECONDS
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
            if access_token(provider, entry.creds.as_deref()).is_some() {
                "yes"
            } else {
                "no"
            }
        ));
    }
    let token = live.or_else(|| access_token(provider, entry.creds.as_deref()));
    match fetch(provider, token.as_deref()) {
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
    access_token(provider, crate::live::creds_raw(provider).as_deref())
}

#[cfg(test)]
mod tests {
    use super::{cache_rolled_over, expired, window_from, window_now, windows_from_models, Usage};
    use serde_json::json;

    fn quota(remaining: f64, resets: &str) -> serde_json::Value {
        json!({ "quotaInfo": { "remainingFraction": remaining, "resetTime": resets } })
    }

    #[test]
    fn the_two_slots_are_the_furthest_gone_family_and_the_freshest() {
        let models = json!({
            "gemini-3-pro": quota(0.1, "2099-01-01T00:00:00Z"),
            "gemini-3-fast": quota(0.8, "2099-01-02T00:00:00Z"),
            "claude-opus": quota(0.45, "2099-01-03T00:00:00Z"),
        });
        let usage = windows_from_models(Some(&models)).expect("a reading");
        let most_used = usage.five_hour.expect("the tightest family");
        let least_used = usage.seven_day.expect("the roomiest family");
        assert_eq!(most_used.utilization, 90.0);
        assert_eq!(most_used.resets_at.as_deref(), Some("2099-01-01T00:00:00Z"));
        assert_eq!(least_used.utilization, 20.0);
        assert_eq!(
            least_used.resets_at.as_deref(),
            Some("2099-01-02T00:00:00Z")
        );
    }

    #[test]
    fn a_single_family_fills_both_slots_with_itself() {
        let models = json!({ "gemini-3-pro": quota(0.25, "2099-01-01T00:00:00Z") });
        let usage = windows_from_models(Some(&models)).expect("a reading");
        assert_eq!(usage.pct("five_hour"), Some(75.0));
        assert_eq!(usage.pct("seven_day"), Some(75.0));
    }

    #[test]
    fn a_model_without_a_quota_block_is_not_a_reading() {
        let models = json!({ "gemini-3-pro": json!({ "displayName": "Pro" }) });
        assert!(windows_from_models(Some(&models)).is_none());
        assert!(windows_from_models(Some(&json!({}))).is_none());
        assert!(windows_from_models(None).is_none());
    }

    #[test]
    fn a_token_is_spent_before_it_lapses_not_after() {
        assert!(expired(&json!({ "expiry": "2000-01-01T00:00:00Z" })));
        assert!(!expired(&json!({ "expiry": "2099-01-01T00:00:00Z" })));
        // A payload that says nothing about its expiry is refreshed rather than
        // trusted: one request is cheaper than a rejected reading.
        assert!(expired(&json!({ "access_token": "a" })));
    }

    #[test]
    fn an_expiry_given_as_a_timestamp_is_read_too() {
        assert!(expired(&json!({ "expiry_timestamp": 946_684_800.0 })));
        assert!(!expired(&json!({ "expiry_timestamp": 4_070_908_800.0 })));
    }

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
    fn the_cap_is_a_plain_threshold() {
        assert!(super::at_cap(98.0, super::FIVE_HOUR_CAP));
        assert!(super::at_cap(98.4, super::FIVE_HOUR_CAP));
        assert!(!super::at_cap(97.9, super::FIVE_HOUR_CAP));
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

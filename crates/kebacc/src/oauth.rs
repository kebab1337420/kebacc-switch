//! Renewing a saved login's token pair.
//!
//! Claude Code's OAuth refresh rotates: the answer to a refresh carries a new
//! refresh token and the one that was sent stops working. A pool that saves a
//! pair once and hands the same pair back on every switch therefore hands back
//! a pair the server has already retired, and the CLI, unable to refresh it,
//! asks for a login. So the pair is renewed here, at the moment of the switch,
//! and what comes back is written into the snapshot before it is written into
//! the CLI's credentials.

use crate::jsonio;
use serde_json::{json, Value};

const TOKEN_URL: &str = "https://console.anthropic.com/v1/oauth/token";
/// Claude Code's public OAuth client. Overridable, so a change on Anthropic's
/// side does not need a release to work around.
const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// How close to expiry a pair is renewed at rather than handed over as it is.
const MARGIN_SECONDS: i64 = 5 * 60;

pub enum Renewal {
    /// The pair had time left on it and was left alone.
    Fresh,
    /// A new pair, as a whole credentials document ready to be written.
    Renewed(String),
    /// The renewal was attempted and did not happen. The pair is handed over
    /// unchanged: the CLI's own refresh may still succeed where this did not.
    Failed(String),
}

fn client_id() -> String {
    std::env::var("KEBACC_SWITCH_OAUTH_CLIENT_ID")
        .ok()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| CLIENT_ID.to_string())
}

fn token_url() -> String {
    std::env::var("KEBACC_SWITCH_OAUTH_TOKEN_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| TOKEN_URL.to_string())
}

pub fn oauth_of(raw: &str) -> Option<Value> {
    let creds: Value = serde_json::from_str(raw).ok()?;
    creds.get("claudeAiOauth").filter(|v| !v.is_null()).cloned()
}

pub fn number(value: &Value, key: &str) -> Option<i64> {
    match value.get(key) {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

pub fn expires_at(raw: &str) -> Option<i64> {
    number(&oauth_of(raw)?, "expiresAt")
}

pub fn access_token(raw: &str) -> Option<String> {
    jsonio::str_of(&oauth_of(raw)?, "accessToken")
}

pub fn refresh_token(raw: &str) -> Option<String> {
    jsonio::str_of(&oauth_of(raw)?, "refreshToken")
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// True when the access token is gone or close enough to gone that handing it
/// over would make the CLI refresh it, which is the thing this avoids.
pub fn stale(raw: &str) -> bool {
    match expires_at(raw) {
        Some(at) => at - now_ms() <= MARGIN_SECONDS * 1000,
        None => true,
    }
}

pub fn renew_if_stale(raw: &str) -> Renewal {
    if !stale(raw) {
        return Renewal::Fresh;
    }
    match renew(raw) {
        Ok(fresh) => Renewal::Renewed(fresh),
        Err(problem) => Renewal::Failed(problem),
    }
}

/// Ask for a new pair and fold it into the credentials document, leaving every
/// other field (the scopes, the subscription, the rate limit tier) as it was.
pub fn renew(raw: &str) -> Result<String, String> {
    let Some(refresh) = refresh_token(raw) else {
        return Err("the saved credentials carry no refresh token".into());
    };
    fold(raw, &post(&refresh)?)
}

fn fold(raw: &str, answer: &Value) -> Result<String, String> {
    let mut creds: Value =
        serde_json::from_str(raw).map_err(|_| "the saved credentials are not JSON".to_string())?;
    let Some(access) = jsonio::str_of(answer, "access_token") else {
        return Err("the token endpoint answered without an access token".into());
    };

    let oauth = jsonio::map_mut(
        creds
            .get_mut("claudeAiOauth")
            .ok_or_else(|| "the saved credentials carry no claudeAiOauth block".to_string())?,
    );
    oauth.insert("accessToken".into(), json!(access));
    if let Some(rotated) = jsonio::str_of(answer, "refresh_token") {
        oauth.insert("refreshToken".into(), json!(rotated));
    }
    if let Some(seconds) = number(answer, "expires_in") {
        oauth.insert("expiresAt".into(), json!(now_ms() + seconds * 1000));
    }
    if let Some(seconds) = number(answer, "refresh_expires_in") {
        oauth.insert(
            "refreshTokenExpiresAt".into(),
            json!(now_ms() + seconds * 1000),
        );
    }
    serde_json::to_string(&creds)
        .map_err(|problem| format!("the new pair did not serialise: {problem}"))
}

/// One retry, and only on the answers that are worth retrying: the token
/// endpoint rate limits, and a switch that gives up on the first 429 is a
/// switch that lands on a login prompt for no reason.
fn post(refresh: &str) -> Result<Value, String> {
    let mut last = String::new();
    for attempt in 0..2 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
        match call(refresh) {
            Ok(value) => return Ok(value),
            Err((problem, retry)) => {
                last = problem;
                if !retry {
                    break;
                }
            }
        }
    }
    Err(last)
}

/// The error flag says whether another go is worth anything.
fn call(refresh: &str) -> Result<Value, (String, bool)> {
    let body = json!({
        "grant_type": "refresh_token",
        "refresh_token": refresh,
        "client_id": client_id(),
    });
    let mut response = agent()
        .post(token_url())
        .header("Content-Type", "application/json")
        .send_json(&body)
        .map_err(|problem| {
            (
                format!("the token endpoint did not answer: {problem}"),
                true,
            )
        })?;

    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().unwrap_or_default();
    if (200..300).contains(&status) {
        return serde_json::from_str::<Value>(&text).map_err(|_| {
            (
                "the token endpoint answered something unreadable".into(),
                false,
            )
        });
    }
    let reason = error_of(&text).unwrap_or_else(|| status.to_string());
    Err((
        format!("the token endpoint answered {status} ({reason})"),
        status == 429 || status >= 500,
    ))
}

fn error_of(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    let error = value.get("error")?;
    if let Value::String(kind) = error {
        return (!kind.is_empty()).then(|| kind.clone());
    }
    jsonio::str_of(error, "type").or_else(|| jsonio::str_of(error, "message"))
}

/// The status of the answer is read here rather than raised as an error,
/// because what the body says (`invalid_grant` against anything else) is the
/// difference between "log in again" and "try again later".
fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .http_status_as_error(false);
    #[cfg(windows)]
    let config = config.tls_config(
        ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build(),
    );
    config.build().new_agent()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creds(expires_at: i64) -> String {
        json!({
            "claudeAiOauth": {
                "accessToken": "old-access",
                "refreshToken": "old-refresh",
                "expiresAt": expires_at,
                "subscriptionType": "max",
            }
        })
        .to_string()
    }

    #[test]
    fn a_pair_with_time_left_is_not_stale() {
        assert!(!stale(&creds(now_ms() + 60 * 60 * 1000)));
    }

    #[test]
    fn a_pair_inside_the_margin_is_stale() {
        assert!(stale(&creds(now_ms() + 60 * 1000)));
        assert!(stale(&creds(now_ms() - 1)));
        assert!(stale(r#"{"claudeAiOauth":{"accessToken":"x"}}"#));
    }

    #[test]
    fn the_refresh_token_is_read_back() {
        assert_eq!(refresh_token(&creds(0)).as_deref(), Some("old-refresh"));
        assert_eq!(refresh_token("{}"), None);
    }

    #[test]
    fn a_renewal_keeps_the_fields_the_answer_says_nothing_about() {
        let answer = json!({
            "access_token": "new-access",
            "refresh_token": "new-refresh",
            "expires_in": 28800,
        });
        let folded = fold(&creds(0), &answer).expect("the answer folds in");
        let oauth = oauth_of(&folded).expect("the block survives");
        assert_eq!(
            jsonio::str_of(&oauth, "accessToken").as_deref(),
            Some("new-access")
        );
        assert_eq!(
            jsonio::str_of(&oauth, "refreshToken").as_deref(),
            Some("new-refresh")
        );
        assert_eq!(
            jsonio::str_of(&oauth, "subscriptionType").as_deref(),
            Some("max")
        );
        assert!(!stale(&folded));
    }

    #[test]
    fn an_answer_that_rotates_nothing_keeps_the_refresh_token() {
        let answer = json!({ "access_token": "new-access", "expires_in": 60 });
        let folded = fold(&creds(0), &answer).expect("the answer folds in");
        assert_eq!(refresh_token(&folded).as_deref(), Some("old-refresh"));
    }

    #[test]
    fn an_answer_with_no_access_token_is_refused() {
        assert!(fold(&creds(0), &json!({ "refresh_token": "new" })).is_err());
    }

    #[test]
    fn an_error_body_is_named_whichever_shape_it_takes() {
        assert_eq!(
            error_of(r#"{"error":{"type":"rate_limit_error"}}"#).as_deref(),
            Some("rate_limit_error")
        );
        assert_eq!(
            error_of(r#"{"error":"invalid_grant"}"#).as_deref(),
            Some("invalid_grant")
        );
        assert_eq!(error_of("not json"), None);
    }
}

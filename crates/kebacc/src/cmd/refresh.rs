use super::Options;
use crate::pool::{Entry, Pool, Trust};
use crate::provider::{self, Provider, ProviderId};
use crate::usage;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_INTERVAL_MS: u128 = 5 * 60 * 1000;

pub fn run(provider: &Provider, opts: &Options) -> i32 {
    let pool = Pool::new(provider).entries();
    let readable: Vec<&Entry> = pool
        .iter()
        .filter(|entry| entry.trust != Trust::Changed && entry.creds.is_some())
        .collect();
    if readable.is_empty() {
        return 0;
    }
    usage::for_entries(provider, &readable, opts.refresh);
    0
}

pub fn nudge() {
    if std::env::var("KEBACC_SWITCH_STATUSLINE_REFRESH")
        .is_ok_and(|flag| crate::cmd::statusline::off(&flag))
    {
        return;
    }
    if !stale() {
        return;
    }
    let claimed = crate::lock::locked(crate::lock::REFRESH, || {
        let stamp = stamp_file();
        if !due(&stamp) {
            return false;
        }
        let _ = std::fs::write(&stamp, now_ms().to_string());
        true
    });
    if claimed == Ok(true) {
        spawn();
    }
}

fn stale() -> bool {
    let window = (interval_ms() / 1000) as i64;
    let store = provider::spec(ProviderId::Claude).store;
    let Some(snapshots) = crate::pool::plain_snapshots(&store) else {
        return false;
    };
    for (_, snapshot) in &snapshots {
        if snapshot.is_null() {
            continue;
        }
        let cache = snapshot.get("usageCache");
        if usage::cache_older_than(cache, window) || usage::cache_rolled_over(cache) {
            return true;
        }
    }
    false
}

fn stamp_file() -> PathBuf {
    provider::state_dir().join("refresh.stamp")
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn interval_ms() -> u128 {
    std::env::var("KEBACC_SWITCH_REFRESH_INTERVAL_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u128>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_INTERVAL_MS)
}

fn due(stamp: &PathBuf) -> bool {
    let Some(last) = std::fs::read_to_string(stamp)
        .ok()
        .and_then(|text| text.trim().parse::<u128>().ok())
    else {
        return true;
    };
    let now = now_ms();
    last > now || now - last >= interval_ms()
}

fn spawn() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(exe);
    command
        .args(["refresh", "-Spawned"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::proc::detach(&mut command);
    let _ = crate::proc::spawn_detached(&mut command);
}

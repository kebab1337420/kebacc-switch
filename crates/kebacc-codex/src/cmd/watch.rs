//! The periodic check Claude Code does not offer.
//!
//! Every hook the CLI has is tied to something the user or the agent does:
//! a session opens, a tool is about to run. A turn spent writing a long answer
//! with no tool call in it fires nothing, and that is exactly the stretch where
//! a quota can die unnoticed — the mid-task hook cannot notice what never calls
//! it.
//!
//! So the hooks start a watcher instead: one detached process per machine that
//! wakes on its own clock and runs the same `auto` the mid-task hook spawns.
//! Nothing here talks to the session; it only moves the saved login, which is
//! what the running CLI reads.
//!
//! It has to stop on its own, since nobody owns it. Two ends: the hooks stamp
//! `session.beat` every time they run, and the watcher gives up once that stamp
//! goes cold, which is what a closed CLI looks like from here. Failing that, it
//! dies of old age.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The watcher's own stamp, so a second one knows not to start.
///
/// Every stamp here is named for this pool. The state directory is the one
/// the Claude half uses, and unsuffixed names would have each half read the
/// other's: the second one to start would find a live watcher, decline to
/// start its own, and leave its pool unwatched for the session.
const BEAT: &str = "watch-codex.beat";
/// Set when something wants the watchers gone — an update landing a new
/// binary, mainly. A watcher started after the stamp ignores it.
const STOP: &str = "watch-codex.stop";
/// The last time a hook ran, which is the last proof a session exists.
const SESSION_BEAT: &str = "session-codex.beat";
/// No hook for this long and the CLI is taken to be gone. Long enough to cover
/// a session sitting idle between two turns, short enough that a watcher does
/// not outlive its terminal by much.
const IDLE_EXIT: Duration = Duration::from_secs(30 * 60);

/// How long the watcher waits on a silent session before giving up.
/// `KEBACC_SWITCH_WATCH_IDLE_MS` moves it, which is how the shutdown gets
/// tested without sitting through half an hour.
fn idle_exit() -> Duration {
    std::env::var("KEBACC_SWITCH_WATCH_IDLE_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or(IDLE_EXIT)
}
/// A watcher never runs longer than this, whatever the stamps say. A stuck
/// process that checks a quota forever is not something to leave behind.
const MAX_LIFE: Duration = Duration::from_secs(12 * 60 * 60);
/// How often the wait between two checks looks up to see whether it has been
/// asked to stop. The check itself is twenty seconds apart; a stop that waited
/// for the next one would have an uninstall sitting there that long, or killing
/// the process instead of asking.
const POLL: Duration = Duration::from_secs(1);

fn state(name: &str) -> PathBuf {
    crate::provider::state_dir().join(name)
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn stamp(name: &str) {
    let _ = std::fs::write(state(name), now_ms().to_string());
}

/// How old a stamp is. Missing, unreadable, or dated in the future all read as
/// "very old": the answer only ever decides whether to start something, and
/// starting a watcher that is not needed costs one process.
fn age(name: &str) -> Duration {
    let Some(then) = std::fs::read_to_string(state(name))
        .ok()
        .and_then(|text| text.trim().parse::<u128>().ok())
    else {
        return Duration::MAX;
    };
    let now = now_ms();
    if then > now {
        return Duration::MAX;
    }
    Duration::from_millis((now - then) as u64)
}

/// The gap between two checks. Shared with the mid-task hook on purpose: the
/// two do the same job from different sides, and one knob for both is one
/// fewer way to end up with a fast hook and a slow watcher.
pub fn interval() -> Duration {
    Duration::from_millis(super::midtask::interval_ms() as u64)
}

/// A watcher is taken to be alive while its stamp is younger than three
/// intervals. Two would be enough on a machine that is not busy; three leaves
/// room for a check that took a while to answer.
fn watcher_alive() -> bool {
    age(BEAT) < interval() * 3
}

/// Called by the hooks. Records that a session is alive and starts the watcher
/// if none is running.
pub fn ensure_running(provider: &str) {
    stamp(SESSION_BEAT);
    if watcher_alive() {
        return;
    }
    // Claim the stamp before spawning, so two hooks firing at once start one
    // watcher and not two.
    stamp(BEAT);
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(exe);
    command
        .args(["watch", "-Provider", provider])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::proc::detach(&mut command);
    let _ = command.spawn();
}

/// Ask every running watcher to stop. Used before a new binary goes in: the old
/// process would otherwise keep checking with the old code until the session
/// ends.
pub fn request_stop() {
    stamp(STOP);
}

/// Ask, then wait for the answer. A watcher takes its stamp away on the way
/// out, so the file going missing is the proof it is gone — no process list to
/// walk, and nothing to kill.
///
/// The stamp existing at all is what starts the wait, not the stamp being
/// fresh: a busy machine can leave a live watcher looking stale for a tick, and
/// answering "already gone" to that is how an uninstall ends with a watcher
/// still switching accounts behind it.
///
/// Answers false when the wait runs out on a stamp that is still being kept
/// warm, which means a watcher is genuinely still there and the caller has to
/// say so.
pub fn stop_and_wait(limit: Duration) -> bool {
    request_stop();
    let beat = state(BEAT);
    if !beat.exists() {
        return true;
    }
    let deadline = SystemTime::now() + limit;
    while SystemTime::now() < deadline {
        if !beat.exists() {
            return true;
        }
        std::thread::sleep(POLL);
    }
    if !beat.exists() {
        return true;
    }
    // The stamp outlived the wait without moving. Only a watcher that was
    // killed rather than asked leaves one like that — a live one rewrites it
    // every tick — so it is swept here, or the next hook would read it and
    // decide a watcher is already on duty.
    if age(BEAT) >= interval() * 3 {
        let _ = std::fs::remove_file(&beat);
        return true;
    }
    false
}

/// Whether a stop was asked for after this watcher started. A stamp older than
/// us belongs to a previous round and is none of our business.
fn stop_requested(started_ms: u128) -> bool {
    std::fs::read_to_string(state(STOP))
        .ok()
        .and_then(|text| text.trim().parse::<u128>().ok())
        .is_some_and(|asked| asked >= started_ms)
}

/// The loop itself. Runs until the session it serves goes quiet.
pub fn run(provider: &str) -> i32 {
    let started = SystemTime::now();
    let started_ms = now_ms();
    loop {
        stamp(BEAT);
        if stop_requested(started_ms) || age(SESSION_BEAT) > idle_exit() {
            return done();
        }
        if started.elapsed().unwrap_or(Duration::ZERO) > MAX_LIFE {
            return done();
        }
        check(provider);
        if !nap(started_ms) {
            return done();
        }
    }
}

/// The wait between two checks, in slices, so a stop asked for during it is
/// answered in a second rather than at the next tick. False means stop now.
fn nap(started_ms: u128) -> bool {
    let mut left = interval();
    while !left.is_zero() {
        let slice = left.min(POLL);
        std::thread::sleep(slice);
        left -= slice;
        if stop_requested(started_ms) {
            return false;
        }
    }
    true
}

/// The stamp is the only sign this process was here, so it goes on the way out:
/// whoever asked for the stop is waiting on exactly that.
fn done() -> i32 {
    let _ = std::fs::remove_file(state(BEAT));
    0
}

/// One check, in a child of its own. The same command the mid-task hook
/// spawns, so a switch from here leaves the same note and reads the same
/// quota; a check that dies takes nothing with it.
fn check(provider: &str) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(exe);
    command
        .args(["auto", "-Provider", provider, "-Hook", "-Spawned"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::proc::hidden(&mut command);
    let _ = command.status();
}

/// Whether a watcher is on duty, for `doctor` to report.
pub fn on_duty() -> bool {
    watcher_alive()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_stamp_reads_as_ancient() {
        assert_eq!(age("kebacc-codex-no-such-stamp"), Duration::MAX);
    }

    #[test]
    fn a_stop_asked_for_before_we_started_is_not_ours() {
        stamp(STOP);
        let later = now_ms() + 60_000;
        assert!(!stop_requested(later));
    }

    #[test]
    fn a_stop_asked_for_since_we_started_stops_us() {
        let started = now_ms();
        stamp(STOP);
        assert!(stop_requested(started));
    }

    #[test]
    fn the_idle_wait_falls_back_to_the_long_one() {
        std::env::remove_var("KEBACC_SWITCH_WATCH_IDLE_MS");
        assert_eq!(idle_exit(), IDLE_EXIT);
    }

    #[test]
    fn the_watcher_shares_the_midtask_interval() {
        assert_eq!(interval().as_millis(), super::super::midtask::interval_ms());
    }
}

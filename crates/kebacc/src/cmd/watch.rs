use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BEAT: &str = "watch.beat";
const STOP: &str = "watch.stop";
const SESSION_BEAT: &str = "session.beat";
const IDLE_EXIT: Duration = Duration::from_secs(30 * 60);

fn idle_exit() -> Duration {
    std::env::var("KEBACC_SWITCH_WATCH_IDLE_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or(IDLE_EXIT)
}
const MAX_LIFE: Duration = Duration::from_secs(12 * 60 * 60);
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
    write_stamp(name, &now_ms().to_string());
}

fn beat() {
    write_stamp(
        BEAT,
        &format!(
            "{} {} {}",
            now_ms(),
            std::process::id(),
            env!("CARGO_PKG_VERSION")
        ),
    );
}

fn write_stamp(name: &str, text: &str) {
    let path = state(name);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, text);
}

pub fn duty() -> Option<(Option<u32>, Option<String>, Duration)> {
    if !watcher_alive() {
        return None;
    }
    let text = std::fs::read_to_string(state(BEAT)).ok()?;
    let mut words = text.split_whitespace().skip(1);
    Some((
        words.next().and_then(|word| word.parse().ok()),
        words.next().map(str::to_string),
        age(BEAT),
    ))
}

fn age(name: &str) -> Duration {
    let Some(then) = std::fs::read_to_string(state(name))
        .ok()
        .and_then(|text| text.split_whitespace().next()?.parse::<u128>().ok())
    else {
        return Duration::MAX;
    };
    let now = now_ms();
    if then > now {
        return Duration::MAX;
    }
    Duration::from_millis((now - then) as u64)
}

pub fn interval() -> Duration {
    Duration::from_millis(super::midtask::interval_ms() as u64)
}

fn watcher_alive() -> bool {
    age(BEAT) < interval() * 3
}

pub fn ensure_running(wanted: &crate::provider::Wanted) {
    stamp(SESSION_BEAT);
    if watcher_alive() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(exe);
    command.arg("watch");
    command.args(wanted.flags());
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::proc::detach(&mut command);
    if crate::proc::spawn_detached(&mut command).is_ok() {
        beat();
    }
}

pub fn request_stop() {
    stamp(STOP);
}

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
    if age(BEAT) >= interval() * 3 {
        let _ = std::fs::remove_file(&beat);
        return true;
    }
    false
}

fn stop_after(stamp: &Path, started_ms: u128) -> bool {
    std::fs::read_to_string(stamp)
        .ok()
        .and_then(|text| text.trim().parse::<u128>().ok())
        .is_some_and(|asked| asked >= started_ms)
}

fn stop_requested(started_ms: u128) -> bool {
    stop_after(&state(STOP), started_ms)
}

pub fn run(wanted: &crate::provider::Wanted) -> i32 {
    let started = SystemTime::now();
    let started_ms = now_ms();
    loop {
        beat();
        if stop_requested(started_ms) || age(SESSION_BEAT) > idle_exit() {
            return done();
        }
        if started.elapsed().unwrap_or(Duration::ZERO) > MAX_LIFE {
            return done();
        }
        check(wanted);
        if !nap(started_ms) {
            return done();
        }
    }
}

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

fn done() -> i32 {
    let _ = std::fs::remove_file(state(BEAT));
    0
}

fn check(wanted: &crate::provider::Wanted) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut command = Command::new(exe);
    command.arg("auto");
    command.args(wanted.flags());
    command.args(["-Hook", "-Spawned"]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::proc::hidden(&mut command);
    let _ = command.status();
}

pub fn on_duty() -> bool {
    watcher_alive()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_stamp_reads_as_ancient() {
        assert_eq!(age("kebacc-no-such-stamp"), Duration::MAX);
    }

    #[test]
    fn a_stop_asked_for_before_we_started_is_not_ours() {
        let (dir, stamp) = unique_stamp("before");
        let _ = std::fs::write(&stamp, now_ms().to_string());
        let later = now_ms() + 60_000;
        assert!(!stop_after(&stamp, later));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stop_asked_for_since_we_started_stops_us() {
        let (dir, stamp) = unique_stamp("since");
        let started = now_ms();
        let _ = std::fs::write(&stamp, now_ms().to_string());
        assert!(stop_after(&stamp, started));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn unique_stamp(label: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "kebacc-watch-{label}-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let stamp = dir.join("stop");
        (dir, stamp)
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

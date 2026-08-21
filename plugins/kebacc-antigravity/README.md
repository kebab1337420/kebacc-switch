# Kebacc antigravity

Several Antigravity logins, saved on this machine, and one
command to move between them when one runs out of quota.

It is a Rust binary: the crate is `crates/kebacc-antigravity` at the root of this
repository, and the binary installs itself — `kebacc-antigravity install` puts it,
the slash commands and the hooks into `~/.claude-tools` and the Claude Code
settings. No binary is committed and there is no third-party
repository in the trust path.
The one thing that is downloaded is the switcher updating itself, from this
repository's own releases — see *Staying up to date*, and the switch that turns
it off.

A command that a status line runs on every repaint, and a hook that runs at every
session start, is a process start the user waits for: the binary answers in a few
milliseconds where the PowerShell it replaces took most of a second to load its
own runtime first.

## What it does

- **add** — saves the login the CLI is using right now into a pool.
- **list** — the saved logins with what is known of their quota. `-Refresh`
  asks the provider's API; without it the numbers come from the cache.
- **switch** — puts another saved login in front of the CLI.
- **auto** — switches only when the one in use is out of quota, and only to one
  that is not. It does nothing on its own: something has to call it, which is
  what `kebacc-antigravity install -AutoSwitch` sets up.
- **remove** — forgets a saved login. The live session is untouched.
- **doctor** — what is installed, what is readable, what the pool thinks of
  itself. `-Protect` re-seals plain-text snapshots, `-Adopt` stamps the ones
  this machine never registered, `-Rollback` puts back the credentials from
  before the last switch, `-Clean` deletes files an earlier version left behind.
- **update** — replaces the installed binary with the newest release.
  `-Check` only says whether one is out.

Every command takes `-Provider antigravity`, which is also the default
to run once per provider.

```
kebacc-antigravity list -Provider all
kebacc-antigravity auto -Provider antigravity
kebacc-antigravity switch -Provider antigravity -Email you@example.com
```

## Staying up to date

The switcher updates itself. At session start, at most once a day, it asks
GitHub for the newest `kebacc-antigravity-v*` release of this repository, and if that
release is newer than what is installed it downloads the binary built for this
platform, checks it against the SHA-256 the release publishes for that asset,
and puts it in place. That happens in a detached process, so the
session does not wait for it, and the running command finishes on the binary it
started on — the new one is used from the next start.

It says so afterwards rather than before, and only when asked: `doctor` reports
the last update for a day after it happened. The status line stays out of it —
the version it prints is the one you are running, which is the thing worth
seeing every second.

```
kebacc-antigravity update -Check   # exit 10 when a newer release exists
kebacc-antigravity update          # install it now
```

`kebacc-antigravity install -NoAutoUpdate` turns this off at install time by writing
`KEBACC_SWITCH_UPDATE=off` into the Claude Code settings. Two environment
variables decide the rest:

- `KEBACC_SWITCH_UPDATE=off` (also `0` or `no`) — never check, never install.
  `update` run by hand says so and does nothing.
- `KEBACC_SWITCH_UPDATE_INTERVAL_MS` — how long between two checks. The default
  is a day.

The update check happens at session start only. The tool-use hook installed by
`-AutoSwitch` never installs anything: a binary must not be replaced underneath a
session that is already running.

## Switching without being asked

`kebacc-antigravity arm -Provider antigravity|off` is what arms it after the
fact — it writes that hook and nothing else, never touching the account in use.
The slash commands `/kebacc-auto-antigravity` and `/kebacc-auto-toggle` are that
command; switching the live login is `/kebacc-switch-antigravity`, and nothing
else.

Two flags for a machine that carries another half as well:

- `-Merge` adds this pool to whatever is already armed instead of replacing it,
  so installing this plugin cannot narrow a hook somebody else widened. The
  installers use it.
- `-Drop` takes this pool out. This build carries one pool, so what is left is
  nothing it could be armed on and the hooks go — the other half's hooks run its
  own binary, under its own name, and are never read or written here. The
  uninstallers use it.

`kebacc-antigravity install -AutoSwitch` writes a pair of hooks into
`~/.claude/settings.json`:

- `SessionStart`, so `auto` runs once as each Claude Code session starts: a
  session that would have opened on a capped account opens on a free one
  instead.
- `PreToolUse`, matching every tool, so an account that runs out *during* a task
  is left behind there and then. Without it a long job sits on a capped account
  until the session ends. This hook does no work of its own: it reads a stamp
  file, and at most once a minute
  (`KEBACC_SWITCH_MIDTASK_INTERVAL_MS`) it spawns a detached `auto`, so the tool
  call it runs in front of is never held up. A tool call that is itself a
  switcher command is skipped — you asked to look, not to move. Naming the
  switcher is not the same as calling it: a `grep kebacc` or an edit to a file
  under `crates/kebacc-antigravity/` still arms the check.

Both hooks fire on something somebody does. A turn spent writing a long answer
with no tool call in it fires neither, and that is a stretch where a quota can
die unnoticed. So the hooks also start a **watcher**: one detached process per
machine that wakes on its own clock, every `KEBACC_SWITCH_MIDTASK_INTERVAL_MS`,
and runs the same `auto`. It never talks to the session — it moves the saved
login, which is what the running CLI reads.

The watcher has to stop on its own, since nothing owns it. Three ways out: the
hooks stamp `session.beat` whenever they run and it gives up once that stamp is
half an hour cold, which is what a closed CLI looks like from here; `update`
asks every watcher to stop before putting a new binary in place; and it never
lives longer than twelve hours whatever the stamps say. `doctor` says whether
one is on duty, and `uninstall` stops it before removing anything.

Installing again replaces those hooks rather than adding more, and
`kebacc-antigravity uninstall` takes both back out.

## The status line

`kebacc-antigravity install -StatusLine` points Claude Code's status line at
`kebacc-antigravity
statusline`, which reads the payload on stdin and prints one line. It draws the account in use and its two windows, how
many saved logins still have room and, when the hooks are in place, what the
switch is armed for:

```
you · 5h 43% / 7d 69% · 2/3 free · auto antigravity
```

An account whose quota has never been read counts as neither free nor capped: it
is added on as `+1?` rather than folded into the free count, so the line does not
promise room it has not seen.

The line it draws is never the network's answer: the live window comes from the
payload Claude Code hands it, the rest from the cache the switcher already
wrote. It keeps that cache moving on its own. The live window is written back
into the account's snapshot, so the other sessions see it without asking
anything, and when a saved account's numbers are more than five minutes old the
draw spawns `kebacc-antigravity refresh` behind itself — a detached process that
reads the quotas, writes them to the snapshots and exits. The draw itself does
not wait for it; the fresher numbers land on the next one. One refresh at a
time, machine-wide, however many sessions are open.

| Variable | What |
| --- | --- |
| `KEBACC_SWITCH_STATUSLINE_REFRESH=off` | never spawn the background refresh; the numbers then only move when you run a command |
| `KEBACC_SWITCH_REFRESH_INTERVAL_MS` | how old the numbers may get before a draw refreshes them, in milliseconds (default 300000) |

Inside Claude Code the same things are slash commands, all under one prefix:
`/kebacc-add-antigravity`, `/kebacc-list-all`, `/kebacc-auto-all`, and so on. The
root [`README.md`](../../README.md) lists every one of them.

## Where things are kept

| Path | What |
| --- | --- |
| `~/.claude-tools/` | the binary, and `.antigravity-version` |
| `~/.kebacc-switch-antigravity-accounts/` | saved Antigravity logins |
| `~/.claude/commands/kebacc-*.md` | the slash commands |
| `~/.kebacc-antigravity/` | stamps, locks and caches (`KEBACC_SWITCH_STATE_DIR` moves it) |

The switching itself has four knobs, all read from the Claude Code settings'
`env` block so the hooks see them:

| Variable | What |
| --- | --- |
| `CLAUDE_AUTOSWITCH_THRESHOLD` | the five-hour window's cap, in percent (default 99). Below it an account counts as having room |
| `CLAUDE_AUTOSWITCH_WEEKLY_THRESHOLD` | the same for the seven-day window (default 99) |
| `KEBACC_SWITCH_MIDTASK_INTERVAL_MS` | how long between two checks, for the mid-task hook and the watcher alike (default 60000) |
| `KEBACC_SWITCH_WATCH_IDLE_MS` | how long the watcher waits on a silent session before giving up (default 1800000) |

A quota reading is cached for a minute, except within ten points of a cap: there
it is only trusted for five seconds. Far from the cap a minute-old number cannot
be wrong in a way that matters, and near it that same minute is the difference
between switching in time and spending a turn on an account that is already
refusing.

Leaving the thresholds at 99 means the switch happens once the account is
practically empty, so a request or two can still land on it before the next
check. Dropping them a few points — 97, say — buys the margin back: the switch
lands while there is still quota to spend.

One saved login is one `.json` file. The dotfiles beside them are the pool's own
bookkeeping.

## How the saved logins are protected

The tokens in a snapshot are sealed before they are written: DPAPI on Windows,
AES-GCM elsewhere under a key held by the macOS Keychain or by libsecret. A
sealed value is `ccx1:` followed by base64. Where no OS secret store exists the
snapshot is written in plain text and every command says so out loud.

A pool directory is just a directory, so anything able to write there could drop
a snapshot in it. Each entry is therefore stamped with an HMAC over the file
name, the account, and a hash of the tokens, under a key only this user can
read. `list`, `switch` and `doctor` report an entry that does not verify;
`switch` asks before using one.

## Requirements

Nothing at run time, and nothing to install with either: the binary carries what
it needs, talks to DPAPI, to the Keychain or to libsecret through whatever the
platform already has, and installs and uninstalls itself. There is no shell
script left in this directory. `install-antigravity.bat`, at the repository root, is the one
exception, and it exists only so a Windows machine can start from a double-click:
it downloads the published binary and runs `kebacc-antigravity install`.

What does need a toolchain is building the crate — or skip that: the releases
carry a binary for Windows, for both kinds of Mac and for x86_64 and arm64
Linux. Download the one for the machine and ask it to install itself.

## Layout

```
src/commands/*.md               the slash commands, carried by the binary
                                 as include_str!, one per entry in COMMANDS
VERSION                         the number the install stamps into
                                 .antigravity-version
crates/kebacc-antigravity/src/main.rs    the entry point, and `-Provider all`
crates/kebacc-antigravity/src/provider.rs   what each CLI keeps on disk
crates/kebacc-antigravity/src/pool.rs    the trust stamps
crates/kebacc-antigravity/src/seal.rs    DPAPI, Keychain, libsecret
crates/kebacc-antigravity/src/usage.rs   the quota windows and their cache
crates/kebacc-antigravity/src/live.rs    the credentials the CLI is holding
crates/kebacc-antigravity/src/cmd/       one file per command, status line included
                                 — install and uninstall too
```

The crate lives in the workspace at the repository root rather than under
`plugins/`, because that is where `cargo build` looks for it. Nothing in this
directory is executed: the `.md` files are read at compile time by `install.rs`
and travel inside the binary, so a command added here reaches nobody until the
binary is built again. CI fails a build where the two lists disagree.

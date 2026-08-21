# Kebacc switch

Several logins for Claude Code, Codex and Antigravity, saved on this
machine, and one command to move between them when one runs out of quota.

It is a Rust binary: the crate is `crates/kebacc` at the root of this
repository, and the binary installs itself — `kebacc install` puts it,
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
  what `kebacc install -AutoSwitch` sets up.
- **remove** — forgets a saved login. The live session is untouched.
- **doctor** — what is installed, what is readable, what the pool thinks of
  itself. `-Protect` re-seals plain-text snapshots, `-Adopt` stamps the ones
  this machine never registered, `-Rollback` puts back the credentials from
  before the last switch, `-Clean` deletes files an earlier version left behind.
- **update** — replaces the installed binary with the newest release.
  `-Check` only says whether one is out.

`list`, `auto` and `doctor` with no flag mean every pool. `add`, `switch` and
`remove` need one: `-claude`/`-cc`, `-codex`/`-cx`, `-antigravity`/`-ag`.

```
kebacc list
kebacc list -ag
kebacc auto -claude
kebacc switch -claude -Email you@example.com
```

## Staying up to date

The switcher updates itself. At session start, at most once a day, it asks
GitHub for the newest `kebacc-v*` release of this repository, and if that
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
kebacc update -Check   # exit 10 when a newer release exists
kebacc update          # install it now
```

`kebacc install -NoAutoUpdate` turns this off at install time by writing
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

`kebacc arm -ag` (or `-claude`, `-codex`, `-all`, `off`) is what arms it after
the fact. It writes that hook and nothing else, never touching the account in
use. The slash command `/kebacc-auto` is that command; switching the live login
is `/kebacc-switch`.

Two flags for a machine that already has a pool armed:

- `-Merge` adds this pool to whatever is already armed instead of replacing it,
  so `kebacc arm -codex -Merge` cannot drop Claude.
- `-Drop` takes this pool out. What is left stays armed. If nothing is left the
  hooks go.

`kebacc install -AutoSwitch` writes a pair of hooks into
`~/.claude/settings.json`:

- `SessionStart`, so `auto` runs once as each Claude Code session starts: a
  session that would have opened on a capped account opens on a free one
  instead.
- `PreToolUse`, matching every tool, so an account that runs out *during* a task
  is left behind there and then. Without it a long job sits on a capped account
  until the session ends. This hook does no work of its own: it reads a stamp
  file, and at most once every twenty seconds
  (`KEBACC_SWITCH_MIDTASK_INTERVAL_MS`) it spawns a detached `auto`, so the
  tool call it runs in front of is never held up. A tool call that is itself a
  switcher command is skipped — you asked to look, not to move. Naming the
  switcher is not the same as calling it: a `grep kebacc` or an edit to a file
  under `crates/kebacc/` still arms the check.

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
`kebacc uninstall` takes both back out.

## The status line

`kebacc install -StatusLine` points Claude Code's status line at
`kebacc
statusline`, which reads the payload on stdin and prints one line. It draws the account in use and its two windows, how
many saved logins still have room and, when the hooks are in place, what the
switch is armed for:

```
you · 5h 43% / 7d 69% · 2/3 free · auto claude
```

An account whose quota has never been read counts as neither free nor capped: it
is added on as `+1?` rather than folded into the free count, so the line does not
promise room it has not seen.

The line it draws is never the network's answer: the live window comes from the
payload Claude Code hands it, the rest from the cache the switcher already
wrote. It keeps that cache moving on its own. The live window is written back
into the account's snapshot, so the other sessions see it without asking
anything, and when a saved account's numbers are more than five minutes old the
draw spawns `kebacc refresh` behind itself, a detached process that
reads the quotas, writes them to the snapshots and exits. The draw itself does
not wait for it; the fresher numbers land on the next one. One refresh at a
time, machine-wide, however many sessions are open.

| Variable | What |
| --- | --- |
| `KEBACC_SWITCH_STATUSLINE_REFRESH=off` | never spawn the background refresh; the numbers then only move when you run a command |
| `KEBACC_SWITCH_REFRESH_INTERVAL_MS` | how old the numbers may get before a draw refreshes them, in milliseconds (default 300000) |

Inside Claude Code the same things are slash commands: `/kebacc-list`,
`/kebacc-add`, `/kebacc-switch`, `/kebacc-remove`, `/kebacc-auto`,
`/kebacc-doctor`, `/kebacc-update`. Pass `-ag` (or `-claude`, `-codex`) as the
argument. The root [`README.md`](../../README.md) lists them.

## Where things are kept

| Path | What |
| --- | --- |
| `~/.claude-tools/` | the binary, and `.version` |
| `~/.kebacc-switch-accounts/` | saved Claude Code logins |
| `~/.kebacc-switch-codex-accounts/` | saved Codex logins |
| `~/.kebacc-switch-antigravity-accounts/` | saved Antigravity logins |
| `~/.claude/commands/kebacc-*.md` | the slash commands |
| `~/.kebacc-switch/` | stamps, locks and caches (`KEBACC_SWITCH_STATE_DIR` moves it) |
| `~/.kebacc-switch/kebacc.log` | what every switch did, rotated at 512 KB (`KEBACC_SWITCH_LOG=off` stops it) |

The switching itself has four knobs, all read from the Claude Code settings'
`env` block so the hooks see them:

| Variable | What |
| --- | --- |
| `CLAUDE_AUTOSWITCH_THRESHOLD` | the five-hour window's cap, in percent (default 98). Below it an account counts as having room |
| `CLAUDE_AUTOSWITCH_WEEKLY_THRESHOLD` | the same for the seven-day window (default 98) |
| `KEBACC_SWITCH_MIDTASK_INTERVAL_MS` | how long between two checks, for the mid-task hook and the watcher alike (default 20000) |
| `KEBACC_SWITCH_WATCH_IDLE_MS` | how long the watcher waits on a silent session before giving up (default 1800000) |

A quota reading is cached for a minute, except within ten points of a cap: there
it is only trusted for five seconds. Far from the cap a minute-old number cannot
be wrong in a way that matters, and near it that same minute is the difference
between switching in time and spending a turn on an account that is already
refusing.

The default of 98 leaves two points of margin, so the switch lands while there
is still quota for the turn in flight rather than once the account is already
refusing. Raising them towards 99 spends more of each account and risks a
request or two landing after the quota is gone; dropping them further — 95, say
— widens the margin at the cost of leaving quota unused.

Crossing the threshold is not a reason to stop working. The switch is what
answers it: the session is told which account it moved to and goes on from
there, and when no account has room the note says so and the next check moves as
soon as one does.

One saved login is one `.json` file. The dotfiles beside them are the pool's own
bookkeeping.

## Keeping a saved login usable

Claude Code rotates its tokens: a refresh returns a new refresh token and
retires the one it was given. A snapshot taken once therefore goes off, and
switching to it hands the CLI a pair the server has already forgotten, which is
what a login prompt right after a switch means.

So a switch does two things beyond copying a file. On the way out it writes the
pair the CLI is using right now back into the snapshot of the login being left,
keeping whatever the session rotated. On the way in it renews a saved pair that
is within five minutes of expiry, and writes what comes back into the snapshot
before handing it over.

When a renewal fails the pair goes over unchanged and the CLI gets its own go at
it; the switch says so, and the reason is in `~/.kebacc-switch/kebacc.log`.
`/kebacc-doctor` reports each saved login's token, and `kebacc doctor -Renew`
renews the ones that have run out. A login whose refresh token is dead needs a
`/login` and `/kebacc-add -claude` once, and stays alive after that.

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
script left in this directory. `install.bat`, at the repository root, is the one
exception, and it exists only so a Windows machine can start from a double-click:
it downloads the published binary and runs `kebacc install`.

What does need a toolchain is building the crate — or skip that: the releases
carry a binary for Windows, for both kinds of Mac and for x86_64 and arm64
Linux. Download the one for the machine and ask it to install itself.

## Layout

```
src/commands/*.md               the slash commands, carried by the binary
                                 as include_str!, one per entry in COMMANDS
VERSION                         the number the install stamps into .version
crates/kebacc/src/main.rs       the entry point, and `-ag` / `-claude` / `-codex`
crates/kebacc/src/provider.rs   what each CLI keeps on disk
crates/kebacc/src/pool.rs       the trust stamps
crates/kebacc-core/src/seal.rs  DPAPI, Keychain, libsecret
crates/kebacc/src/usage.rs      the quota windows and their cache
crates/kebacc/src/live.rs       the credentials the CLI is holding
crates/kebacc/src/cmd/          one file per command, status line included
```

The crate lives in the workspace at the repository root rather than under
`plugins/`, because that is where `cargo build` looks for it. Nothing in this
directory is executed: the `.md` files are read at compile time by `install.rs`
and travel inside the binary, so a command added here reaches nobody until the
binary is built again. CI fails a build where the two lists disagree.

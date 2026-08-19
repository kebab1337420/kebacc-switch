# Kebacc switch

Several logins for Claude Code, saved on this machine, and one
command to move between them when one runs out of quota.

It is a Rust binary: the crate is `crates/kebacc-switch` at the root of this
repository, and `install.ps1` copies what came out of `cargo build --release`
into `~/.claude-tools`. No binary is committed and there is no third-party
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
  what `install.ps1 -AutoSwitch` sets up.
- **remove** — forgets a saved login. The live session is untouched.
- **doctor** — what is installed, what is readable, what the pool thinks of
  itself. `-Protect` re-seals plain-text snapshots, `-Adopt` stamps the ones
  this machine never registered, `-Rollback` puts back the credentials from
  before the last switch, `-Clean` deletes files an earlier version left behind.
- **update** — replaces the installed binary with the newest release.
  `-Check` only says whether one is out.

Every command takes `-Provider claude`, which is also the default
to run once per provider.

```
kebacc-switch list -Provider all
kebacc-switch auto -Provider claude
kebacc-switch switch -Provider claude -Email you@example.com
```

## Staying up to date

The switcher updates itself. At session start, at most once a day, it asks
GitHub for the newest `kebacc-switch-v*` release of this repository, and if that
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
kebacc-switch update -Check   # exit 10 when a newer release exists
kebacc-switch update          # install it now
```

`install.ps1 -NoAutoUpdate` turns this off at install time by writing
`KEBACC_SWITCH_UPDATE=off` into the Claude Code settings. Two environment
variables decide the rest:

- `KEBACC_SWITCH_UPDATE=off` (also `0` or `no`) — never check, never install.
  `update` run by hand says so and does nothing.
- `KEBACC_SWITCH_UPDATE_INTERVAL_MS` — how long between two checks. The default
  is a day.

The check happens at session start only. The tool-use hook installed by
`-AutoSwitch` never installs anything: a binary must not be replaced underneath a
session that is already running.

## Switching without being asked

`kebacc-switch arm -Provider claude|off` is what arms it after the
fact — it writes that hook and nothing else, never touching the account in use.
The slash commands `/kebacc-auto-claude` and `/kebacc-auto-toggle` are that
command; switching the live login is `/kebacc-switch-claude`, and nothing
else.

`install.ps1 -AutoSwitch claude` writes a pair of hooks into
`~/.claude/settings.json`:

- `SessionStart`, so `auto` runs once as each Claude Code session starts: a
  session that would have opened on a capped account opens on a free one
  instead.
- `PreToolUse`, matching every tool, so an account that runs out *during* a task
  is left behind there and then. Without it a long job sits on a capped account
  until the session ends. This hook does no work of its own: it reads a stamp
  file, and at most once every five minutes
  (`KEBACC_SWITCH_MIDTASK_INTERVAL_MS`) it spawns a detached `auto`, so the tool
  call it runs in front of is never held up. A tool call that is itself a
  switcher command is skipped — you asked to look, not to move.

Installing again replaces those hooks rather than adding more, and
`uninstall.ps1` takes both back out. There is no watcher and no daemon: apart from
the short refresh the status line spawns for itself, between two sessions
nothing of this is running.

## The status line

`install.ps1 -StatusLine` points Claude Code's status line at `kebacc-switch
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
draw spawns `kebacc-switch refresh` behind itself — a detached process that
reads the quotas, writes them to the snapshots and exits. The draw itself does
not wait for it; the fresher numbers land on the next one. One refresh at a
time, machine-wide, however many sessions are open.

| Variable | What |
| --- | --- |
| `KEBACC_SWITCH_STATUSLINE_REFRESH=off` | never spawn the background refresh; the numbers then only move when you run a command |
| `KEBACC_SWITCH_REFRESH_INTERVAL_MS` | how old the numbers may get before a draw refreshes them, in milliseconds (default 300000) |

Inside Claude Code the same things are slash commands, all under one prefix:
`/kebacc-add-claude`, `/kebacc-list-all`, `/kebacc-auto-all`, and so on. The
root [`README.md`](../../README.md) lists every one of them.

## Where things are kept

| Path | What |
| --- | --- |
| `~/.claude-tools/` | the binary, and `.version` |
| `~/.kebacc-switch-accounts/` | saved Claude Code logins |
| `~/.claude/commands/kebacc-*.md` | the slash commands |
| `~/.kebacc-switch/` | stamps, locks and caches (`KEBACC_SWITCH_STATE_DIR` moves it) |

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

Nothing at run time: the binary carries what it needs, and it talks to DPAPI, to
the Keychain or to libsecret through whatever the platform already has.
PowerShell 7 (`pwsh`) is needed to run `install.ps1` and `uninstall.ps1` on
Windows; on macOS and Linux the same work is done by `install.sh` and
`uninstall.sh`, which need nothing beyond a POSIX shell. A Rust toolchain is
needed only to build the crate: the releases carry a binary for Windows, for
both kinds of Mac and for x86_64 and arm64 Linux, and `bootstrap.sh` fetches the
right one.

## Layout

```
install.ps1 / uninstall.ps1     put it down, take it back, on Windows
install.sh / uninstall.sh       the same, on macOS and Linux
bootstrap.ps1 / bootstrap.sh    install from a release, with no clone
src/commands/*.md               the slash commands
crates/kebacc-switch/src/main.rs    the entry point, and `-Provider all`
crates/kebacc-switch/src/provider.rs   what each CLI keeps on disk
crates/kebacc-switch/src/pool.rs    the trust stamps
crates/kebacc-switch/src/seal.rs    DPAPI, Keychain, libsecret
crates/kebacc-switch/src/usage.rs   the quota windows and their cache
crates/kebacc-switch/src/live.rs    the credentials the CLI is holding
crates/kebacc-switch/src/cmd/       one file per command, status line included
```

The crate lives in the workspace at the repository root rather than under
`plugins/`, because that is where `cargo build` looks for it. `install.ps1`
finds the built binary two directories up from itself, under `target/`.

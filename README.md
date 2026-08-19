<div align="center">
⚠️⚠️⚠️⚠️⚠️⚠️⚠️(MADE FOR BOITE : https://github.com/beboite/boite)⚠️⚠️⚠️⚠️⚠️⚠️⚠️


# kebacc-codex

**Several Codex logins on one machine, and one command to move between them
when the one you are on runs out of quota.**

[![release](https://img.shields.io/github/v/release/kebab1337420/kebacc-switch?sort=semver&label=release)](https://github.com/kebab1337420/kebacc-switch/releases)
[![rust](https://img.shields.io/badge/rust-2021-orange)](https://www.rust-lang.org)
[![platform](https://img.shields.io/badge/platform-windows%20%C2%B7%20macos%20%C2%B7%20linux-blue)](#install)

</div>

---

## What it is

A small Rust binary and a Claude Code plugin. It saves the Codex login you are
on, keeps the saved ones sealed on disk, reads each account's quota from the
API, and moves you to one that still has room — on its own, at session start
and again while a task runs, before you notice you were capped.

Claude Code is the host: the slash commands, the session hooks and the status
line are its. The pool is Codex's, in `~/.codex/auth.json`. Nothing here reads
or writes a Claude login.

```
crates/kebacc-codex/    the binary
plugins/kebacc-codex/    the installers, the hooks, the slash commands
```

The Claude pool is a separate switcher, `kebacc-switch`, on the `master` branch
and published under its own `kebacc-switch-v*` tags. Both can sit on one
machine: different binary names, different version markers, different release
tags, and each one reads and rewrites only the hooks that run its own binary.

---

## Install

### Windows, no toolchain

Download **`install-codex.bat`** from the
[latest release](https://github.com/kebab1337420/kebacc-switch/releases) and run
it. It fetches the published binary and the plugin, installs both, and needs no
clone, no Rust and no administrator. Arguments go through to the installer, so
`install-codex.bat -StatusLine -AutoSwitch` works.

`bootstrap.ps1`, published beside it on the same release, is what it runs — a
plain script you can read first if you would rather see what it does. It comes
from the release rather than from the branch on purpose: raw file URLs are
served through a cache that can be minutes behind, and an installer that
sometimes runs yesterday's code is worse than one pinned to a release. A change
to it therefore needs a fresh upload to reach anyone.

### macOS and Linux, no toolchain

```sh
curl -fsSL https://github.com/kebab1337420/kebacc-switch/releases/download/kebacc-codex-v0.2.6/bootstrap.sh | sh
```

It picks the binary for the machine it is on — Apple silicon, Intel Macs,
x86_64 or arm64 Linux — unpacks the plugin from the source archive of that tag,
and runs `install.sh`. Options go through after `sh -s --`, so
`sh -s -- --status-line --auto-switch` works. Only `curl` and `tar` are needed.

### From source

```sh
cargo build --release
pwsh -NoProfile -File plugins/kebacc-codex/install.ps1   # Windows
sh plugins/kebacc-codex/install.sh                       # macOS, Linux
```

The installer copies `target/release/kebacc-codex` into `~/.claude-tools`, then
writes the hooks, the status line and the slash commands into your Claude Code
settings. It backs the settings file up first and refuses to write a result that
would not parse. `uninstall.ps1`, or `uninstall.sh`, takes all of it back out.

| Windows | macOS, Linux | Effect |
| --- | --- | --- |
| `-StatusLine` | `--status-line` | point the Claude Code status line at the switcher |
| `-AutoSwitch` | `--auto-switch` | run `auto` at every session start |
| `-NoAutoUpdate` | `--no-auto-update` | leave the daily self-update off |
| `-NoProfileEdit` | `--no-profile-edit` | do not touch the shell profile |

---

## Slash commands

Everything the plugin installs lives under the `/kebacc-` prefix.

| Command | What it does |
| --- | --- |
| `/kebacc-add-codex` | save the Codex login in `~/.codex/auth.json` |
| `/kebacc-list-codex` | what is saved, and what is known of each quota |
| `/kebacc-switch-codex` | put another saved login in front of the CLI |
| `/kebacc-remove-codex` | forget a saved login; the live session is untouched |
| `/kebacc-auto-codex` | arm or disarm the automatic switch |
| `/kebacc-doctor-codex` | check the install, the pool and the session hook |

A list always asks the API rather than reading the cache, and prints both quota
windows with the time left until each one resets.

Arming changes nothing about the account in use: it decides what the *next*
sessions open on. Only `/kebacc-switch-codex` moves the login you are on right
now.

---

## The binary

The slash commands are thin wrappers; the binary takes the same work directly.

```sh
kebacc-codex add                        # save the current login
kebacc-codex list -Refresh -Countdown
kebacc-codex switch -Email you@example.com
kebacc-codex auto                       # switch only if capped
kebacc-codex arm -Provider codex        # arm the session-start switch, change nothing now
kebacc-codex arm -Provider off          # disarm it
kebacc-codex doctor
kebacc-codex refresh                    # re-read the quotas, print nothing
kebacc-codex update
```

`-Provider` survives because the slash commands and the hooks pass it. This
build carries one pool, so `codex` is what it takes and what it defaults to;
`claude` is refused with a line saying where that pool lives.

**Arming**

`arm` is the only command that writes to the Claude Code settings, and it only
ever touches hooks that run `kebacc-codex`. A Claude switcher installed beside
this one arms its own pair, under its own binary, and neither uninstaller can
disarm the other:

```sh
kebacc-codex arm -Provider codex -Merge   # add this pool to what our hook carries
kebacc-codex arm -Provider codex -Drop    # take it back out, disarm when nothing is left
```

`all` is still understood as a scope: a hook left by the version where one
binary carried both pools says `all`, and this build reads that as its own
pool rather than as something it no longer has.

**Exit codes**

| Code | Meaning |
| --- | --- |
| `0` | nothing to do |
| `1` | a problem, reported on a `!` line |
| `2` | no identity to save |
| `10` | switched |
| `20` | every saved account is capped |
| `30` | fewer than two accounts saved |
| `64` | unknown command |

---

## Where things live

| Path | What |
| --- | --- |
| `~/.claude-tools/` | the installed binary, and `.codex-version` — this half's marker |
| `~/.claude/commands/kebacc-*-codex.md` | the slash commands |
| `~/.kebacc-switch/` | locks, stamps, update state |
| `~/.kebacc-switch-codex-accounts/` | the saved logins |
| `~/.codex/auth.json` | the login the Codex CLI is using right now |

`KEBACC_SWITCH_CODEX_ACCOUNTS` moves the pool, `CODEX_HOME` is where the CLI's
own file is looked for.

Saved credentials are sealed before they touch disk: DPAPI on Windows, and
AES-256-GCM under a key held by the macOS Keychain or by libsecret elsewhere.
Each snapshot carries an HMAC-SHA256 stamp, so a pool file edited outside the
tool is reported as changed rather than trusted.

The full account of what is stored and what the hooks do is in
[`plugins/kebacc-codex/README.md`](plugins/kebacc-codex/README.md).

---

## Self-update

At session start, at most once a day, the switcher asks this repository's
releases whether a newer `kebacc-codex-v*` tag exists and installs it in the
background. `KEBACC_SWITCH_UPDATE=off` stops that, and so does installing with
`-NoAutoUpdate`.

Building with `KEBACC_SWITCH_RELEASES_REPO=owner/name` set points the updater
somewhere else at compile time.

---

## Development

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

### Building a release binary

rustc records the absolute path of every source file it compiles into the
binary's panic metadata, which on a normal machine means the builder's home
directory and username travel with every download. Published binaries are built
with those paths remapped:

```powershell
$flags = @(
    "--remap-path-prefix=$env:USERPROFILE\.cargo=/cargo",
    "--remap-path-prefix=$env:USERPROFILE=/home/build",
    "--remap-path-prefix=$PWD=/src"
)
$env:CARGO_ENCODED_RUSTFLAGS = ($flags -join "`u{001f}")
cargo build --release -p kebacc-codex
```

`CARGO_ENCODED_RUSTFLAGS` rather than `RUSTFLAGS` because the separator is a
unit separator instead of a space, so a checkout in a directory whose name has
spaces in it still works. The flags are not in a committed `config.toml`: they
name a path that is different on every machine.

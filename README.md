<div align="center">
(MADE FOR BOITE : https://github.com/beboite/boite)


# kebacc-switch 

**Several Claude Code and Codex logins on one machine, and one command to move
between them when the one you are on runs out of quota.**

[![release](https://img.shields.io/github/v/release/kebab1337420/kebacc-switch?sort=semver&label=release)](https://github.com/kebab1337420/kebacc-switch/releases)
[![rust](https://img.shields.io/badge/rust-2021-orange)](https://www.rust-lang.org)
[![platform](https://img.shields.io/badge/platform-windows%20%C2%B7%20macos%20%C2%B7%20linux-blue)](#build-and-install)

</div>

---

## What it is

A small Rust binary and a Claude Code plugin. It saves the login you are on,
keeps the saved ones sealed on disk, reads each account's quota from the API,
and moves you to one that still has room — on its own, at session start, before
you notice you were capped.

```
crates/kebacc-switch/    the binary
plugins/kebacc-switch/   the installer, the hooks and the slash commands
```

---

## Build and install

```sh
cargo build --release
pwsh -NoProfile -File plugins/kebacc-switch/install.ps1
```

The installer copies `target/release/kebacc-switch` into `~/.claude-tools`, then
writes the hooks, the status line and the slash commands into your Claude Code
settings. It backs the settings file up first and refuses to write a result that
would not parse. `uninstall.ps1` takes all of it back out.

| Flag | Effect |
| --- | --- |
| `-NoAutoUpdate` | leave the daily self-update off |
| `-NoProfileEdit` | do not touch the PowerShell profile |

---

## Slash commands

Everything the plugin installs lives under the `/kebacc-` prefix.

### Accounts

| Command | What it does |
| --- | --- |
| `/kebacc-add-claude` | save the Claude Code login you are on right now |
| `/kebacc-add-codex` | save the Codex login you are on right now |
| `/kebacc-remove-claude` | forget a saved Claude Code account |
| `/kebacc-remove-codex` | forget a saved Codex account |

### Looking

| Command | What it does |
| --- | --- |
| `/kebacc-list-all` | every saved account, both providers |
| `/kebacc-list-claude` | the saved Claude Code accounts |
| `/kebacc-list-codex` | the saved Codex accounts |

A list command always asks the API rather than reading the cache, and always
prints both quota windows with the time left until each one resets. There is
nothing to pass and nothing else to run.

### Moving

| Command | What it does |
| --- | --- |
| `/kebacc-switch-claude` | change which saved Claude Code login the CLI uses |
| `/kebacc-switch-codex` | change which saved Codex login the CLI uses |

### The auto-switch

| Command | What it does |
| --- | --- |
| `/kebacc-auto-all` | check both providers, switch only where there is no room left |
| `/kebacc-auto-claude` | the same, Claude Code only |
| `/kebacc-auto-codex` | the same, Codex only |
| `/kebacc-auto-toggle` | arm or disarm the session-start hook |

### Upkeep

| Command | What it does |
| --- | --- |
| `/kebacc-doctor` | check the install, the pool and the seals |
| `/kebacc-update` | install the newest release |

---

## The binary

The slash commands are thin wrappers; the binary takes the same work directly.

```sh
kebacc-switch add     -Provider claude               # save the current login
kebacc-switch list    -Provider all -Refresh -Countdown
kebacc-switch switch  -Provider claude -Email you@example.com
kebacc-switch auto    -Provider all                  # switch only if capped
kebacc-switch doctor  -Provider all
kebacc-switch update
```

`-Provider` takes `claude`, `codex` or `all`, and defaults to `claude`.

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
| `~/.claude-tools/` | the installed binary and its `.version` |
| `~/.claude/commands/kebacc-*.md` | the slash commands |
| `~/.kebacc-switch/` | locks, stamps, update state |
| `~/.kebacc-switch-accounts/` | the Claude Code pool |
| `~/.kebacc-switch-codex-accounts/` | the Codex pool |

Saved credentials are sealed before they touch disk: DPAPI on Windows, and
AES-256-GCM under a key held by the macOS Keychain or by libsecret elsewhere.
Each snapshot carries an HMAC-SHA256 stamp, so a pool file edited outside the
tool is reported as changed rather than trusted.

The full account of what is stored and what the hooks do is in
[`plugins/kebacc-switch/README.md`](plugins/kebacc-switch/README.md).

---

## Self-update

At session start, at most once a day, the switcher asks this repository's
releases whether a newer `kebacc-switch-v*` tag exists and installs it in the
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

The crate carries no comments: the code is meant to read without them, and the
prose that explains a decision goes in a commit message or in this README.

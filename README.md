<div align="center">

# kebacc

Several logins on one machine, and one command to move between them when
the one you are on runs out of quota. Built for
[Boite](https://github.com/beboite/boite).

[![release](https://img.shields.io/github/v/release/kebab1337420/kebacc-switch?sort=semver&label=release)](https://github.com/kebab1337420/kebacc-switch/releases)
[![rust](https://img.shields.io/badge/rust-2021-orange)](https://www.rust-lang.org)
[![platform](https://img.shields.io/badge/platform-windows%20%C2%B7%20macos%20%C2%B7%20linux-blue)](#install)

</div>

---

## What it is

One Rust binary and one Claude Code plugin. Claude Code, Codex and Antigravity
each keep their own pool of sealed logins. You install once, then name the pool
with a flag:

```
kebacc install
kebacc list
kebacc list -ag
kebacc switch -claude
kebacc switch -codex
kebacc add -ag
```

Full name or short: `-claude`/`-cc`, `-codex`/`-cx`, `-antigravity`/`-ag`,
`-grok`/`-gk`, `-all`.
`list`, `auto`, `doctor` and `arm` with no flag mean every pool. `add`,
`switch`, `remove`, `set` and `use` need one. Uninstall takes the binary and the slash commands. The saved
logins stay until you pass `-Pool`. An install or update rewrites leftover
`-Provider` hooks and sweeps the old per-pool slash commands.

Grok publishes no usage of its own, so it is saved, listed, switched and given
a session directory like any other pool, but without numbers: `auto` passes over
it, and `set -FiveHour` and `-SevenDay` are refused for it.

---

## Install

One installer. It downloads the published `kebacc` binary by the `kebacc-v*`
tag prefix, never GitHub's Latest label. Leftover `kebacc-codex-v*` and
`kebacc-antigravity-v*` tags from when each pool was its own release can still
be the newest GitHub Latest.

### Windows, no toolchain

Download `install.bat` from [the matching release](https://github.com/kebab1337420/kebacc-switch/releases)
and run it. Arguments go through to the installer, so
`install.bat -StatusLine -AutoSwitch all` works. No clone, no Rust, no
administrator.

The asset is taken through the GitHub API rather than through the plain
download URL, which is served by a cache that keeps handing out the previous
file for a while after an asset is replaced.

### macOS and Linux, no toolchain

One file, downloaded and asked to install itself. Pick the name for the
machine you are on (`aarch64-apple-darwin`, `x86_64-apple-darwin`,
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`):

```sh
name=kebacc-aarch64-apple-darwin
url=$(curl -fsSL https://api.github.com/repos/kebab1337420/kebacc-switch/releases |
  grep -o "https://[^\"]*/$name" | head -n 1)
curl -fsSL "$url" -o /tmp/kebacc && chmod +x /tmp/kebacc
/tmp/kebacc install --status-line --auto-switch all
```

With the `gh` CLI on the machine, the same thing is two lines. Name the tag
prefix, not `latest`:

```sh
gh release download --repo kebab1337420/kebacc-switch --pattern kebacc-aarch64-apple-darwin --dir /tmp
chmod +x /tmp/kebacc-* && /tmp/kebacc-* install
```

The binary installs itself into `~/.claude-tools` and takes the slash commands
with it, so the copy in `/tmp` can go afterwards.

### From source

```sh
cargo build --release
./target/release/kebacc install
```

The installer copies the binary into `~/.claude-tools`, then writes the hooks,
the status line and every slash command into your Claude Code settings. It
backs the settings file up first and refuses to write a result that would not
parse. `kebacc uninstall` takes all of that back out. Leftover `kebacc-codex`
and `kebacc-antigravity` binaries from the old split are swept on install.

Every option is spelled both ways. `-StatusLine` and `--status-line` reach the
same flag, so a habit from either shell works:

| Option | Effect |
| --- | --- |
| `-StatusLine` | point the Claude Code status line at the switcher |
| `-AutoSwitch all` | run `auto` at session start and mid-task, every pool |
| `-NoAutoUpdate` | leave the daily self-update off |
| `-NoProfileEdit` | do not touch the shell profile |
| `-ToolsDir <dir>` | install somewhere other than `~/.claude-tools` |

---

## Slash commands

Ten commands. The pool is an argument, same flags as the binary.

| Command | What it does |
| --- | --- |
| `/kebacc-list` | saved accounts. `/kebacc-list -ag` for Antigravity only |
| `/kebacc-add` | save the login you are on. Needs `-ag`, `-claude`, `-codex` or `-grok` |
| `/kebacc-switch` | put a saved login in front |
| `/kebacc-remove` | forget a saved login |
| `/kebacc-auto` | arm the auto-switch. `/kebacc-auto off` disarms it |
| `/kebacc-doctor` | check the install and the pools |
| `/kebacc-status` | what is live, what it has left, what is armed |
| `/kebacc-set` | per-pool settings: rank, reserve, thresholds, on-switch command |
| `/kebacc-use` | set a session directory up on one account |
| `/kebacc-update` | install the newest release |

A list command always asks the API rather than reading the cache, and always
prints both quota windows with the time left until each one resets. `/kebacc-auto`
only writes hooks; `/kebacc-switch` is what moves the login you are on.

---

## The binary

The slash commands are thin wrappers; the binary takes the same work directly.

```sh
kebacc add -ag                          # save the current Antigravity login
kebacc list -Refresh -Countdown         # every pool
kebacc list -ag
kebacc switch -claude -Email you@example.com
kebacc auto                             # switch only if capped, every pool
kebacc arm -ag                          # arm Antigravity, change nothing now
kebacc arm -claude -Merge               # add Claude to whatever is already armed
kebacc arm -ag -Drop                    # take Antigravity out, leave the rest
kebacc arm off
kebacc doctor
kebacc doctor -claude -Renew            # ask for a new token pair for the logins whose own has run out
kebacc refresh -codex                   # re-read that pool, print nothing
kebacc update
```

`-claude`/`-cc`, `-codex`/`-cx`, `-antigravity`/`-ag`, `-grok`/`-gk`, `-all`. No flag on list,
auto, doctor, arm, watch and refresh means every pool. add, switch, remove, set
and use need one.

### Exit codes

| Code | Meaning |
| --- | --- |
| `0` | nothing to do |
| `1` | a problem, reported on a `!` line |
| `2` | no identity to save |
| `10` | switched, or `update -Check` found a newer release |
| `20` | every saved account is capped |
| `30` | fewer than two accounts saved |
| `64` | unknown command |

---

## Where things live

| Path | What |
| --- | --- |
| `~/.claude-tools/` | the installed binary |
| `~/.claude-tools/.version` | which version is installed |
| `~/.claude/commands/kebacc-*.md` | the slash commands |
| `~/.kebacc-switch/` | locks, stamps, update state |
| `~/.kebacc-switch/kebacc.log` | what every switch did |
| `~/.kebacc-switch-accounts/` | the Claude Code pool |
| `~/.kebacc-switch-codex-accounts/` | the Codex pool |
| `~/.kebacc-switch-antigravity-accounts/` | the Antigravity pool |
| `~/.kebacc-switch-grok-accounts/` | the Grok pool |

Saved credentials are sealed before they touch disk: DPAPI on Windows, and
AES-256-GCM under a key held by the macOS Keychain or by libsecret elsewhere.
Each snapshot carries an HMAC-SHA256 stamp, so a pool file edited outside the
tool is reported as changed rather than trusted.

The keychain account names are load-bearing. Claude and Codex store the seal
key under `kebacc-switch`. Antigravity stores it under `kebacc-antigravity`.
Renaming either unlocks nothing already sealed.

The full account of what is stored and what the hooks do is in
[`plugins/kebacc/README.md`](plugins/kebacc/README.md).

---

## Tokens, and why a switch used to end in a login prompt

Claude Code's OAuth refresh rotates. Every time the CLI renews its access
token, the answer carries a new refresh token and the one it sent stops
working. That is what makes a saved login perishable: a snapshot taken once
holds a pair the server retires the next time the CLI refreshes, so switching
back hands the CLI a pair it cannot use, and the CLI asks for a login.

Two things keep the pool alive:

- **On the way out**, the pair the CLI is using right now is written back into
  the snapshot of the login being left. Whatever the CLI rotated during the
  session is kept rather than thrown away.
- **On the way in**, a saved pair inside five minutes of expiry is renewed
  against the token endpoint first, and what comes back is written into the
  snapshot before it is written into the CLI's credentials.

A renewal that fails is not fatal: the saved pair goes over as it was, the CLI
gets its own go at refreshing it, and the switch says out loud that a login
prompt is possible. `kebacc doctor -Renew` does the same renewal for every
saved login on demand, and reports which ones need a fresh `/login`.

Every switch is written down in `~/.kebacc-switch/kebacc.log`: which login,
which pair (as the first ten hex characters of its SHA-256, never the token),
when it expires, what the renewal answered. `KEBACC_SWITCH_LOG=off` turns it
off; the file is rotated at 512 KB.

| Variable | What it does |
| --- | --- |
| `KEBACC_SWITCH_LOG=off` | stop writing the log |
| `KEBACC_SWITCH_OAUTH_CLIENT_ID` | the OAuth client the renewal names |
| `KEBACC_SWITCH_OAUTH_TOKEN_URL` | where the renewal is sent |

---

## Self-update

At session start, at most once a day, the switcher asks this repository's
releases whether a newer `kebacc-v*` tag exists and installs it in the
background. `KEBACC_SWITCH_UPDATE=off` stops that, and so does installing with
`-NoAutoUpdate`.

Building with `KEBACC_SWITCH_RELEASES_REPO=owner/name` set points the updater
somewhere else at compile time.

---

## Development

```sh
cargo fmt --all -- --check
cargo clippy --release --all-targets --workspace -- -D warnings
cargo test --release --workspace
cargo build --release
```

The crates carry comments only where a decision is not in the code. The prose
that explains a decision goes in a commit message or in this README.

### Building a release binary

rustc records the absolute path of every source file it compiles into the
binary's panic metadata, which on a normal machine means the builder's home
directory and username travel with every download. Published binaries are
built with those paths remapped:

```powershell
$flags = @(
    "--remap-path-prefix=$env:USERPROFILE\.cargo=/cargo",
    "--remap-path-prefix=$env:USERPROFILE=/home/build",
    "--remap-path-prefix=$PWD=/src"
)
$env:CARGO_ENCODED_RUSTFLAGS = ($flags -join "`u{001f}")
cargo build --release --workspace
```

`CARGO_ENCODED_RUSTFLAGS` rather than `RUSTFLAGS` because the separator is a
unit separator instead of a space, so a checkout in a directory whose name has
spaces in it still works. The flags are not in a committed `config.toml`: they
name a path that is different on every machine.

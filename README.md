<div align="center">
⚠️⚠️⚠️⚠️⚠️⚠️⚠️(MADE FOR BOITE : https://github.com/beboite/boite)⚠️⚠️⚠️⚠️⚠️⚠️⚠️


# kebacc-antigravity

**Several Antigravity logins on one machine, and one command to move between them
when the one you are on runs out of quota.**

[![release](https://img.shields.io/github/v/release/kebab1337420/kebacc-switch?sort=semver&label=release)](https://github.com/kebab1337420/kebacc-switch/releases)
[![rust](https://img.shields.io/badge/rust-2021-orange)](https://www.rust-lang.org)
[![platform](https://img.shields.io/badge/platform-windows%20%C2%B7%20macos%20%C2%B7%20linux-blue)](#install)

</div>

---

## What it is

A small Rust binary and a Claude Code plugin. It saves the Antigravity login you are
on, keeps the saved ones sealed on disk, reads each account's quota from the
API, and moves you to one that still has room — on its own, at session start
and again while a task runs, before you notice you were capped.

Claude Code is the host: the slash commands, the session hooks and the status
line are its. The pool is Antigravity's. Nothing here reads or writes a Claude
login.

Antigravity keeps one login in two places, and this writes both, because a
switch that moved only one would leave the other half signed in as somebody
else:

- the CLI (`agy`) reads `~/.gemini/antigravity-cli/antigravity-oauth-token`
- the IDE reads the operating system's credential store, under service
  `gemini` and user `antigravity` — Credential Manager on Windows, the
  Keychain on macOS, libsecret elsewhere

The credential store is best effort: a machine with no entry, or a locked
keyring, still gets the file written and the switch still counts.
`KEBACC_SWITCH_NO_KEYRING=1` leaves the store alone entirely.

> The IDE reads its login when it starts. Switch while it is running and it
> keeps using the account it opened on until it is restarted. The newer
> in-IDE state (`state.vscdb`) is deliberately not touched.

```
crates/kebacc-antigravity/     the binary
plugins/kebacc-antigravity/    the slash commands and the version marker
```

The Claude pool is a separate switcher, `kebacc-switch`, on the `master` branch
and published under its own `kebacc-switch-v*` tags. Both can sit on one
machine: different binary names, different version markers, different release
tags, and each one reads and rewrites only the hooks that run its own binary.

---

## Install

### Windows, no toolchain

Download **`install-antigravity.bat`** from the
[latest release](https://github.com/kebab1337420/kebacc-switch/releases) and run
it. It downloads the published binary and asks it to install itself, and needs
no clone, no Rust and no administrator. Arguments go through to the installer,
so `install-antigravity.bat -StatusLine -AutoSwitch` works.

There is nothing else to fetch: the slash commands travel inside the binary. The
asset is taken through the GitHub API rather than through the plain download
URL, which is served by a cache that keeps handing out the previous file for a
while after an asset is replaced. It asks for the newest release tagged
`kebacc-antigravity-v*`, since the same repository publishes the Claude half
under its own tags.

### macOS and Linux, no toolchain

One file, downloaded and asked to install itself. Pick the name for the machine
you are on — `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`:

```sh
name=kebacc-antigravity-aarch64-apple-darwin
url=$(curl -fsSL https://api.github.com/repos/kebab1337420/kebacc-switch/releases |
  grep -o "https://[^\"]*/$name" | head -n 1)
curl -fsSL "$url" -o /tmp/kebacc-antigravity && chmod +x /tmp/kebacc-antigravity
/tmp/kebacc-antigravity install --status-line --auto-switch
```

With the `gh` CLI on the machine, the same thing is two lines:

```sh
gh release download --repo kebab1337420/kebacc-switch --pattern kebacc-antigravity-aarch64-apple-darwin --dir /tmp
chmod +x /tmp/kebacc-antigravity-* && /tmp/kebacc-antigravity-* install
```

The binary installs itself into `~/.claude-tools` and takes the slash commands
with it, so the copy in `/tmp` can go afterwards.

### From source

```sh
cargo build --release -p kebacc-antigravity
./target/release/kebacc-antigravity install
```

The installer copies the binary into `~/.claude-tools`, then writes the hooks,
the status line and the slash commands into your Claude Code settings. It backs
the settings file up first and refuses to write a result that would not parse.
`kebacc-antigravity uninstall` takes all of it back out.

Every option is spelled both ways — `-StatusLine` and `--status-line` reach the
same flag, so a habit from either shell works:

| Option | Effect |
| --- | --- |
| `-StatusLine` | point the Claude Code status line at the switcher |
| `-AutoSwitch` | run `auto` at session start and mid-task |
| `-NoAutoUpdate` | leave the daily self-update off |
| `-NoProfileEdit` | do not touch the shell profile |
| `-ToolsDir <dir>` | install somewhere other than `~/.claude-tools` |

---

## Slash commands

Everything the plugin installs lives under the `/kebacc-` prefix.

| Command | What it does |
| --- | --- |
| `/kebacc-add-antigravity` | save the Antigravity login in `~/.gemini/antigravity-cli/antigravity-oauth-token` |
| `/kebacc-list-antigravity` | what is saved, and what is known of each quota |
| `/kebacc-switch-antigravity` | put another saved login in front of the CLI |
| `/kebacc-remove-antigravity` | forget a saved login; the live session is untouched |
| `/kebacc-auto-antigravity` | arm or disarm the automatic switch |
| `/kebacc-doctor-antigravity` | check the install, the pool and the session hook |
| `/kebacc-update-antigravity` | install the newest release of this half |

A list always asks the API rather than reading the cache, and prints both
readings with the time left until each one resets.

Antigravity does not meter an account in fixed windows the way Codex does.
It gives each model family an allowance of its own, with its own reset, and
they empty at very different rates. So the two readings shown are **max**,
the family that is furthest gone — which is what stops work first and what
the auto-switch acts on — and **min**, the family with the most left, which
is what work falls back to. Quota comes from Google's Cloud Code endpoint,
the same one the IDE asks.

Arming changes nothing about the account in use: it decides what the *next*
sessions open on. Only `/kebacc-switch-antigravity` moves the login you are on right
now.

---

## The binary

The slash commands are thin wrappers; the binary takes the same work directly.

```sh
kebacc-antigravity add                        # save the current login
kebacc-antigravity list -Refresh -Countdown
kebacc-antigravity switch -Email you@example.com
kebacc-antigravity auto                       # switch only if capped
kebacc-antigravity arm -Provider antigravity        # arm the session-start switch, change nothing now
kebacc-antigravity arm -Provider off          # disarm it
kebacc-antigravity doctor
kebacc-antigravity refresh                    # re-read the quotas, print nothing
kebacc-antigravity update
```

`-Provider` survives because the slash commands and the hooks pass it. This
build carries one pool, so `antigravity` is what it takes and what it defaults to;
`claude` is refused with a line saying where that pool lives.

**Arming**

`arm` is the only command that writes to the Claude Code settings, and it only
ever touches hooks that run `kebacc-antigravity`. A Claude switcher installed beside
this one arms its own pair, under its own binary, and neither uninstaller can
disarm the other:

```sh
kebacc-antigravity arm -Provider antigravity -Merge   # add this pool to what our hook carries
kebacc-antigravity arm -Provider antigravity -Drop    # take it back out, disarm when nothing is left
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
| `~/.claude-tools/` | the installed binary, and `.antigravity-version` — this half's marker |
| `~/.claude/commands/kebacc-*-antigravity.md` | the slash commands |
| `~/.kebacc-switch/` | locks, stamps, update state |
| `~/.kebacc-switch-antigravity-accounts/` | the saved logins |
| `~/.gemini/antigravity-cli/antigravity-oauth-token` | the login the Antigravity CLI is using right now |
| `gemini:antigravity` in the OS credential store | the same login, as the IDE reads it |

`KEBACC_SWITCH_ANTIGRAVITY_ACCOUNTS` moves the pool, `ANTIGRAVITY_HOME` is
where the CLI's own file is looked for, and `KEBACC_SWITCH_NO_KEYRING=1`
keeps the credential store out of it.

Saved credentials are sealed before they touch disk: DPAPI on Windows, and
AES-256-GCM under a key held by the macOS Keychain or by libsecret elsewhere.
Each snapshot carries an HMAC-SHA256 stamp, so a pool file edited outside the
tool is reported as changed rather than trusted.

The full account of what is stored and what the hooks do is in
[`plugins/kebacc-antigravity/README.md`](plugins/kebacc-antigravity/README.md).

---

## Self-update

At session start, at most once a day, the switcher asks this repository's
releases whether a newer `kebacc-antigravity-v*` tag exists and installs it in the
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
cargo build --release -p kebacc-antigravity
```

`CARGO_ENCODED_RUSTFLAGS` rather than `RUSTFLAGS` because the separator is a
unit separator instead of a space, so a checkout in a directory whose name has
spaces in it still works. The flags are not in a committed `config.toml`: they
name a path that is different on every machine.

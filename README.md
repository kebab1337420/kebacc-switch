<div align="center">
⚠️⚠️⚠️⚠️⚠️⚠️⚠️( MADE FOR BOITE : https://github.com/beboite/boite )⚠️⚠️⚠️⚠️⚠️⚠️⚠️


# kebacc-antigravity 

**Several Antigravity logins on one machine, and one command to move
between them when the one you are on runs out of quota.**

[![release](https://img.shields.io/github/v/release/kebab1337420/kebacc-switch?sort=semver&label=release)](https://github.com/kebab1337420/kebacc-switch/releases)
[![rust](https://img.shields.io/badge/rust-2021-orange)](https://www.rust-lang.org)
[![platform](https://img.shields.io/badge/platform-windows%20%C2%B7%20macos%20%C2%B7%20linux-blue)](#install)

</div>

---

## What it is

A small Rust binary and a Claude Code plugin. It saves the Antigravity login
you are on,
seals saved logins when an OS secret store is available, reads each account's quota from the API,
and moves you to one that still has room — on its own, at session start and
again mid-task, before you notice you were capped.

```
crates/kebacc-antigravity/    the binary
plugins/kebacc-antigravity/   the slash commands, compiled into the binary
```

---

## Install

### Windows, no toolchain

Download **`install-antigravity.bat`** from the
[latest release](https://github.com/kebab1337420/kebacc-switch/releases) and run
it. It downloads the published binary and asks it to install itself, and needs
no clone, no Rust and no administrator. Arguments go through to the installer,
so `install-antigravity.bat -StatusLine -AutoSwitch all` works.

There is nothing else to fetch: the slash commands travel inside the binary. The
asset is taken through the GitHub API rather than through the plain download
URL, which is served by a cache that keeps handing out the previous file for a
while after an asset is replaced.

### macOS and Linux, no toolchain

One file, downloaded and asked to install itself. Pick the name for the machine
you are on — `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`:

The other halves are published from this repository too, so the list of
releases is read rather than the `latest` endpoint: the file is picked by its
own name, whichever half was released last.

```sh
name=kebacc-antigravity-aarch64-apple-darwin
url=$(curl -fsSL https://api.github.com/repos/kebab1337420/kebacc-switch/releases |
  grep -o "https://[^\"]*/$name" | head -n 1)
curl -fsSL "$url" -o /tmp/kebacc-antigravity && chmod +x /tmp/kebacc-antigravity
/tmp/kebacc-antigravity install --status-line --auto-switch antigravity
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
cargo build --release
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
| `-AutoSwitch antigravity` | run `auto` at session start and mid-task |
| `-NoAutoUpdate` | leave the daily self-update off |
| `-NoProfileEdit` | do not touch the shell profile |
| `-ToolsDir <dir>` | install somewhere other than `~/.claude-tools` |

---

## Slash commands

Everything the plugin installs lives under the `/kebacc-` prefix.

### Accounts

| Command | What it does |
| --- | --- |
| `/kebacc-add-antigravity` | save the Antigravity login you are on right now |
| `/kebacc-remove-antigravity` | forget a saved Antigravity account |

### Looking

| Command | What it does |
| --- | --- |
| `/kebacc-list-antigravity` | the saved Antigravity accounts |
| `/kebacc-list-all` | the same, plus every other pool installed beside it |

A list command always asks the API rather than reading the cache, and always
prints both quota windows with the time left until each one resets. There is
nothing to pass and nothing else to run.

### Moving

| Command | What it does |
| --- | --- |
| `/kebacc-switch-antigravity` | change which saved Antigravity login the CLI uses |

### The auto-switch

| Command | What it does |
| --- | --- |
| `/kebacc-auto-antigravity` | arm the auto-switch, session start and mid-task |
| `/kebacc-auto-all` | the same, plus every other pool installed beside it |
| `/kebacc-auto-toggle` | arm or disarm both auto hooks |

Neither of these changes the account in use. Arming decides what the *next*
sessions open on. Only `/kebacc-switch-antigravity` moves the login you are on right
now.

### The `-all` commands

`/kebacc-list-all` and `/kebacc-auto-all` are the only two commands that reach
past the Antigravity pool. Each provider lives on its own branch of this
repository and installs its own plugin, its own binary and its own pool —
Claude Code on `master`, Codex on the `Codex` branch, and whatever follows on a
branch of its own. The `-all` commands walk whichever of those are installed on the machine
and do the same work on each, so a second provider costs no second command.
With only this plugin installed they behave exactly like their `-antigravity`
counterparts.

### Upkeep

| Command | What it does |
| --- | --- |
| `/kebacc-doctor-antigravity` | check the install, the pool and the seals |
| `/kebacc-update-antigravity` | install the newest release |

---

## The binary

The slash commands are thin wrappers; the binary takes the same work directly.

```sh
kebacc-antigravity add     -Provider antigravity               # save the current login
kebacc-antigravity list    -Provider all -Refresh -Countdown
kebacc-antigravity switch  -Provider antigravity -Email you@example.com
kebacc-antigravity auto    -Provider all                  # switch only if capped
kebacc-antigravity arm     -Provider antigravity                # arm the auto-switch, change nothing now
kebacc-antigravity arm     -Provider antigravity -Merge         # arm it without narrowing what is already armed
kebacc-antigravity arm     -Provider antigravity -Drop          # take this pool out again
kebacc-antigravity arm     -Provider off                   # disarm it
kebacc-antigravity doctor  -Provider all
kebacc-antigravity refresh -Provider all                  # re-read the quotas, print nothing
kebacc-antigravity update
```

`-Provider` takes `antigravity` — the only pool this binary knows — and
defaults to it. `all` is still accepted, as a spelling of `antigravity`, so the
hooks written when one binary carried every pool keep working.

**The other halves**

Claude Code lives on `master` as `kebacc-switch`, Codex in `kebacc-codex` on
the `Codex` branch. Each has its own binary, its own pool and its own slash
commands, and they install side by side into the same `~/.claude-tools`. Every
half is published from this repository under a tag prefix of its own, so a
release is found by name rather than by the "Latest" label, which only one of
them can carry.

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
| `~/.claude-tools/` | the installed binary and its `.antigravity-version` |
| `~/.claude/commands/kebacc-*.md` | the slash commands |
| `~/.kebacc-switch/` | locks, stamps, update state, shared with the other halves |
| `~/.kebacc-switch-antigravity-accounts/` | the Antigravity pool |

Windows seals each saved login with DPAPI and needs no extra tool. macOS
and Linux seal with AES-256-GCM under a 32-byte key created on first use
and kept by `security` or `secret-tool`. Each backend also wraps
`.pool.key` in the accounts directory, and the pool stamps each saved
account with HMAC-SHA256 under the key inside that file. When none of
those backends is available, `add` writes the credentials as plain JSON,
skips the stamp, and prints a yellow line once. `doctor` then reports the
backend as none and every account as unverified. Deleting `.pool.key`
also makes every saved account read as unverified.

| Platform | Backend |
| --- | --- |
| Windows | DPAPI |
| macOS | Keychain (`security`) |
| Linux | libsecret (`secret-tool`) |
| none of those | plain JSON, no HMAC stamp |

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

The crate carries no comments: the code is meant to read without them, and the
prose that explains a decision goes in a commit message or in this README.

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

<div align="center">
⚠️⚠️⚠️⚠️⚠️⚠️⚠️( MADE FOR BOITE : https://github.com/beboite/boite )⚠️⚠️⚠️⚠️⚠️⚠️⚠️


# kebacc-switch 

**Several Claude Code logins on one machine, and one command to move
between them when the one you are on runs out of quota.**

[![release](https://img.shields.io/github/v/release/kebab1337420/kebacc-switch?sort=semver&label=release)](https://github.com/kebab1337420/kebacc-switch/releases)
[![rust](https://img.shields.io/badge/rust-2021-orange)](https://www.rust-lang.org)
[![platform](https://img.shields.io/badge/platform-windows%20%C2%B7%20macos%20%C2%B7%20linux-blue)](#install)

</div>

---

## What it is

A small Rust binary and a Claude Code plugin. It saves the login you are on,
keeps the saved ones sealed on disk, reads each account's quota from the API,
and moves you to one that still has room — on its own, at session start and
again mid-task, before you notice you were capped.

```
crates/kebacc-switch/    the binary
plugins/kebacc-switch/   the slash commands, compiled into the binary
```

---

## Install

### Windows, no toolchain

Download **`install.bat`** from the
[latest release](https://github.com/kebab1337420/kebacc-switch/releases) and run
it. It downloads the published binary and asks it to install itself, and needs
no clone, no Rust and no administrator. Arguments go through to the installer,
so `install.bat -StatusLine -AutoSwitch all` works.

There is nothing else to fetch: the slash commands travel inside the binary. The
asset is taken through the GitHub API rather than through the plain download
URL, which is served by a cache that keeps handing out the previous file for a
while after an asset is replaced.

### macOS and Linux, no toolchain

One file, downloaded and asked to install itself. Pick the name for the machine
you are on — `aarch64-apple-darwin`, `x86_64-apple-darwin`,
`x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`:

Releases of the Codex half go out of this repository as well, so the `latest`
endpoint below is the newest `kebacc-switch-v*` release only because that half
is the one that keeps the label. Ask for a tag by name to be sure of what you
get.

```sh
name=kebacc-switch-aarch64-apple-darwin
url=$(curl -fsSL https://api.github.com/repos/kebab1337420/kebacc-switch/releases/latest |
  grep -o "https://[^\"]*/$name")
curl -fsSL "$url" -o /tmp/kebacc-switch && chmod +x /tmp/kebacc-switch
/tmp/kebacc-switch install --status-line --auto-switch claude
```

With the `gh` CLI on the machine, the same thing is two lines:

```sh
gh release download --repo kebab1337420/kebacc-switch --pattern kebacc-switch-aarch64-apple-darwin --dir /tmp
chmod +x /tmp/kebacc-switch-* && /tmp/kebacc-switch-* install
```

The binary installs itself into `~/.claude-tools` and takes the slash commands
with it, so the copy in `/tmp` can go afterwards.

### From source

```sh
cargo build --release
./target/release/kebacc-switch install
```

The installer copies the binary into `~/.claude-tools`, then writes the hooks,
the status line and the slash commands into your Claude Code settings. It backs
the settings file up first and refuses to write a result that would not parse.
`kebacc-switch uninstall` takes all of it back out.

Every option is spelled both ways — `-StatusLine` and `--status-line` reach the
same flag, so a habit from either shell works:

| Option | Effect |
| --- | --- |
| `-StatusLine` | point the Claude Code status line at the switcher |
| `-AutoSwitch claude` | run `auto` at session start and mid-task |
| `-NoAutoUpdate` | leave the daily self-update off |
| `-NoProfileEdit` | do not touch the shell profile |
| `-ToolsDir <dir>` | install somewhere other than `~/.claude-tools` |

---

## Slash commands

Everything the plugin installs lives under the `/kebacc-` prefix.

### Accounts

| Command | What it does |
| --- | --- |
| `/kebacc-add-claude` | save the Claude Code login you are on right now |
| `/kebacc-remove-claude` | forget a saved Claude Code account |

### Looking

| Command | What it does |
| --- | --- |
| `/kebacc-list-claude` | the saved Claude Code accounts |
| `/kebacc-list-all` | the same, plus every other pool installed beside it |

A list command always asks the API rather than reading the cache, and always
prints both quota windows with the time left until each one resets. There is
nothing to pass and nothing else to run.

### Moving

| Command | What it does |
| --- | --- |
| `/kebacc-switch-claude` | change which saved Claude Code login the CLI uses |

### The auto-switch

| Command | What it does |
| --- | --- |
| `/kebacc-auto-claude` | arm the auto-switch, session start and mid-task |
| `/kebacc-auto-all` | the same, plus every other pool installed beside it |
| `/kebacc-auto-toggle` | arm or disarm both auto hooks |

Neither of these changes the account in use. Arming decides what the *next*
sessions open on. Only `/kebacc-switch-claude` moves the login you are on right
now.

### The `-all` commands

`/kebacc-list-all` and `/kebacc-auto-all` are the only two commands that reach
past the Claude Code pool. Each provider lives on its own branch of this
repository and installs its own plugin, its own binary and its own pool —
Codex on the `Codex` branch today, and whatever follows it on a branch of its
own. The `-all` commands walk whichever of those are installed on the machine
and do the same work on each, so a second provider costs no second command.
With only this plugin installed they behave exactly like their `-claude`
counterparts.

### Upkeep

| Command | What it does |
| --- | --- |
| `/kebacc-doctor` | check the install, the pool and the seals |
| `/kebacc-update` | install the newest release |
| `/kebacc-install-codex` | build and install the Codex plugin from its branch |

---

## The binary

The slash commands are thin wrappers; the binary takes the same work directly.

```sh
kebacc-switch add     -Provider claude               # save the current login
kebacc-switch list    -Provider all -Refresh -Countdown
kebacc-switch switch  -Provider claude -Email you@example.com
kebacc-switch auto    -Provider all                  # switch only if capped
kebacc-switch arm     -Provider claude                # arm the auto-switch, change nothing now
kebacc-switch arm     -Provider claude -Merge         # arm it without narrowing what is already armed
kebacc-switch arm     -Provider claude -Drop          # take this pool out again
kebacc-switch arm     -Provider off                   # disarm it
kebacc-switch doctor  -Provider all
kebacc-switch refresh -Provider all                  # re-read the quotas, print nothing
kebacc-switch update
```

`-Provider` takes `claude` — the only pool this binary knows — and defaults to
it. `all` is still accepted, as a spelling of `claude`, so the hooks written
before Codex moved out keep working.

**Codex**

Codex lives in its own plugin, `kebacc-codex`, on the `Codex` branch of this
repository. It has its own binary, its own pool and its own slash commands
(`/kebacc-add-codex`, `/kebacc-list-codex`, `/kebacc-switch-codex`,
`/kebacc-remove-codex`, `/kebacc-auto-codex`), and the two install side by
side. It is published from this repository too, under `kebacc-codex-v*` tags,
with an `install-codex.bat` of its own attached to the release. Both halves are
picked by tag prefix rather than by the "Latest" label, which only one of them
can carry.

`/kebacc-install-codex` clones the branch, builds it with cargo and runs its
installer, which is still the way to install a Codex half newer than its last
release; the same thing by hand is `kebacc-switch install-codex`.

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
cargo build --release -p kebacc-switch
```

`CARGO_ENCODED_RUSTFLAGS` rather than `RUSTFLAGS` because the separator is a
unit separator instead of a space, so a checkout in a directory whose name has
spaces in it still works. The flags are not in a committed `config.toml`: they
name a path that is different on every machine.

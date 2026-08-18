# kebacc-switch

Several logins for Claude Code and for Codex, saved on one machine, and one
command to move between them when the one in use runs out of quota.

```
crates/kebacc-switch/    the binary
plugins/kebacc-switch/   the installer and the slash commands
```

## Build and install

```
cargo build --release
pwsh -NoProfile -File plugins/kebacc-switch/install.ps1
```

`install.ps1` finds `target/release/kebacc-switch.exe`, copies it into
`~/.claude-tools`, and writes the hooks, the status line and the slash commands
into the Claude Code settings. It backs the settings file up first and refuses
to write a result that would not parse. `uninstall.ps1` takes it back out.

Pass `-NoAutoUpdate` to the installer to leave the daily self-update off.

## What it does

```
kebacc-switch add                  save the login the CLI is using right now
kebacc-switch list                 the saved logins and their quota
kebacc-switch list -Refresh        ask the API instead of reading the snapshots
kebacc-switch list -Countdown      both windows of every account, with resets
kebacc-switch switch -Email …      change which saved login the CLI uses
kebacc-switch auto                 switch only if the one in use is out of quota
kebacc-switch doctor               check the install and the pool
kebacc-switch update               install the newest release
```

Exit codes: 0 nothing to do, 1 a problem, 2 no identity to save, 10 switched,
20 every account capped, 30 fewer than two accounts, 64 unknown command.

The full account of what is stored, how it is sealed, and what the hooks do is
in [`plugins/kebacc-switch/README.md`](plugins/kebacc-switch/README.md).

## Where things live

```
~/.claude-tools/                    the installed binary
~/.kebacc-switch/                   locks, stamps, update state
~/.kebacc-switch-accounts/          the Claude Code pool
~/.kebacc-switch-codex-accounts/    the Codex pool
```

Saved credentials are sealed before they touch disk: DPAPI on Windows, and
AES-256-GCM under a key held by the macOS keychain or libsecret elsewhere. Each
snapshot carries an HMAC-SHA256 stamp, so a pool file edited outside the tool is
reported as changed rather than trusted.

## Self-update

At session start, at most once a day, the switcher asks this repository's
releases whether a newer `kebacc-switch-v*` tag exists and installs it in the
background. `KEBACC_SWITCH_UPDATE=off` stops that, and so does installing with
`-NoAutoUpdate`.

Building with `KEBACC_SWITCH_RELEASES_REPO=owner/name` set points the updater
somewhere else at compile time.

## Development

```
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

The crate carries no comments: the code is meant to read without them, and the
prose that explains a decision goes in a commit message or in this README.

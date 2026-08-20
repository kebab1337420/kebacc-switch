# Kebacc codex

Several Codex logins, saved on this machine, and one command to move between
them when one runs out of quota.

The plugin is a Claude Code plugin — that is where the slash commands, the
session hooks and the status line live — but the pool is Codex's:
`~/.codex/auth.json`, saved into `~/.kebacc-switch-codex-accounts/`. No Claude
login is read or written anywhere in it.

## Installing

There is no install script: the binary carries the slash commands inside it and
installs itself. Nothing here needs a clone or a Rust toolchain.

**Windows** — double-click `install-codex.bat`. It downloads the published
binary and runs `kebacc-codex install`, passing on whatever it was given:

```
install-codex.bat -AutoSwitch -StatusLine
```

**macOS and Linux** — download the binary for this platform from the newest
`kebacc-codex-v*` release and ask it to install itself:

```
curl -fsSL -o kebacc-codex https://github.com/kebab1337420/kebacc-switch/releases/latest/download/kebacc-codex-x86_64-unknown-linux-gnu
chmod +x kebacc-codex
./kebacc-codex install --auto-switch
```

From a clone it is the same command on the build you just made:

```
cargo build --release -p kebacc-codex
target/release/kebacc-codex install --auto-switch
```

`--auto-switch` (`-AutoSwitch`) arms the switch, `--status-line`
(`-StatusLine`) points the Claude Code status line here, and `--tools-dir`
(`-ToolsDir`) installs somewhere other than `~/.claude-tools`. Running it again
is how it updates.

## What it does

Inside Claude Code the commands are slash commands:

| Command | What it does |
| --- | --- |
| `/kebacc-add-codex` | save the Codex login in `~/.codex/auth.json` |
| `/kebacc-list-codex` | what is saved, and what is known of each quota |
| `/kebacc-switch-codex` | put another saved login in front of the CLI |
| `/kebacc-remove-codex` | forget a saved login; the live session is untouched |
| `/kebacc-auto-codex` | arm or disarm the automatic switch |
| `/kebacc-doctor-codex` | check the install, the pool and the session hook |
| `/kebacc-update-codex` | install the newest release of this half |

From a shell it is the same binary under its own name:

```
kebacc-codex list
kebacc-codex switch -Email you@example.com
kebacc-codex auto
```

## Switching without being asked

`--auto-switch` writes two hooks into `~/.claude/settings.json`, both running
the same `auto`:

- `SessionStart`, so a session that would have opened on a capped account opens
  on a free one instead.
- `PreToolUse`, matching every tool, so an account that runs out *during* a task
  is switched off partway through it rather than at the next session. That one
  reads a stamp file and does nothing at all unless the last check was more than
  five minutes ago (`KEBACC_SWITCH_MIDTASK_INTERVAL_MS`).

Both hooks run `kebacc-codex` by name, and arming reads and rewrites nothing
else: the Claude switcher, `kebacc-switch`, arms its own pair under its own
binary, and the two never touch each other's. That is what `install` and
`uninstall` call:

```
kebacc-codex arm -Provider codex -Merge   # add this pool to our hook's scope
kebacc-codex arm -Provider codex -Drop    # take it out, disarm when nothing is left
```

`-Provider all` is still read as a scope covering this pool, so a hook written
by the older build — where one binary carried both pools — keeps working until
it is rearmed.

## Uninstalling

`kebacc-codex uninstall` takes the Codex commands out, narrows the hooks, drops
this half's version marker and removes the binary. It asks first; `--yes`
(`-Yes`) answers for you. What the settings name is what it takes back: a second
install of this half, somewhere else on the machine, keeps its own settings and
its own slash commands.

The commands that span both pools — `/kebacc-list-all`, `/kebacc-auto-all` —
go with whichever half leaves last, since they mean nothing on their own.

On Windows a running binary cannot delete itself, so the uninstall leaves a copy
of itself in the temporary directory and that copy takes the name a moment after
this process lets go of it.

The saved logins are kept: they are the point of the tool, and removing the
plugin is not a reason to lose them. `--pool` (`-Pool`) deletes them.

## Where things are kept

| Path | What |
| --- | --- |
| `~/.claude-tools/` | the binary, and `.codex-version` — this half's marker |
| `~/.kebacc-switch-codex-accounts/` | saved Codex logins |
| `~/.claude/commands/kebacc-*-codex.md` | the slash commands |
| `~/.codex/auth.json` | the login the Codex CLI is using right now |

`KEBACC_SWITCH_CODEX_ACCOUNTS` moves the pool, `CODEX_HOME` is where the CLI's
own file is looked for.

The tokens in a saved snapshot are sealed before they touch disk — DPAPI on
Windows, AES-256-GCM under a key held by the macOS Keychain or by libsecret
elsewhere — and each snapshot carries an HMAC-SHA256 stamp, so one dropped into
the pool by anything else is reported rather than used.

## Layout

```
src/commands/*.md         the slash commands this half owns
src/commands-all/*.md     the pair that only means anything with both halves here
VERSION                   the number the marker file carries
```

Nothing else: the shell scripts that used to put this down and take it back are
`kebacc-codex install` and `kebacc-codex uninstall`, in
`crates/kebacc-codex/src/cmd/`. The command files above are compiled into the
binary with `include_str!`, so the binary that installs them is the only thing
that has to arrive on the machine, and a command file that CI finds on disk but
not in that list fails the build.

What another switcher sharing the machine has to answer the same way lives in
the same crate: the version markers that say who is installed
(`.version` for the Claude half, `.codex-version` for this one), the
binary-version guard, and the scope algebra in `cmd/arm.rs` the session hook is
widened and narrowed by.

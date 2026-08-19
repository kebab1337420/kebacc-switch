# Kebacc codex

Several Codex logins, saved on this machine, and one command to move between
them when one runs out of quota.

The plugin is a Claude Code plugin — that is where the slash commands, the
session hooks and the status line live — but the pool is Codex's:
`~/.codex/auth.json`, saved into `~/.kebacc-switch-codex-accounts/`. No Claude
login is read or written anywhere in it.

## Installing

Nothing here needs a clone or a Rust toolchain: the installers fetch the binary
from this repository's releases.

**Windows** — double-click `install-codex.bat`, or:

```
pwsh -NoProfile -File bootstrap.ps1 -AutoSwitch
```

**macOS and Linux**:

```
curl -fsSL https://github.com/kebab1337420/kebacc-switch/releases/download/kebacc-codex-v0.2.6/bootstrap.sh | sh -s -- --auto-switch
```

From a clone, with the binary built (`cargo build --release -p kebacc-codex`),
the plugin installs itself: `install.ps1` on Windows, `install.sh` on macOS and
Linux. Both take `--force` (`-Force`) to install a rebuild of the version that
is already there, and `--auto-switch` (`-AutoSwitch`) to arm the switch.

Running either again is how it updates.

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
binary, and the two never touch each other's. That is what the installers and
uninstallers call:

```
kebacc-codex arm -Provider codex -Merge   # add this pool to our hook's scope
kebacc-codex arm -Provider codex -Drop    # take it out, disarm when nothing is left
```

`-Provider all` is still read as a scope covering this pool, so a hook written
by the older build — where one binary carried both pools — keeps working until
it is rearmed.

## Uninstalling

`uninstall.ps1` on Windows, `uninstall.sh` on macOS and Linux. Either takes the
Codex commands out, narrows the hooks, and drops this half's version marker. The
binary, the status line and the shell function stay if another half is still
installed — whichever leaves last takes them.

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
install.ps1 / uninstall.ps1     put it down, take it back, on Windows
install.sh / uninstall.sh       the same, on macOS and Linux
bootstrap.ps1 / bootstrap.sh    install from a release, with no clone
shared.ps1 / shared.sh          what a switcher has to agree on to share a machine
src/commands/*.md               the slash commands
```

`shared.ps1` and `shared.sh` hold the part another switcher on the same machine
has to answer the same way: the version markers that say who is installed, the
binary-version guard, and the scope algebra the session hook is widened and
narrowed by.

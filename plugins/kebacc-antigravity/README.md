# Kebacc antigravity

Several Antigravity logins, saved on this machine, and one command to move between
them when one runs out of quota.

The plugin is a Claude Code plugin — that is where the slash commands, the
session hooks and the status line live — but the pool is Antigravity's, saved
into `~/.kebacc-switch-antigravity-accounts/`. No Claude login is read or
written anywhere in it.

Antigravity keeps one login in two places, and a switch writes both:
`~/.gemini/antigravity-cli/antigravity-oauth-token`, which the `agy` CLI reads,
and the operating system's credential store under service `gemini` and user
`antigravity`, which the IDE reads. The store is best effort — a machine
without an entry, or with a locked keyring, still gets the file written and the
switch still counts — and `KEBACC_SWITCH_NO_KEYRING=1` leaves it alone.

> The IDE reads its login when it starts, so a switch made while it is running
> only takes effect once it is restarted. The newer in-IDE state (`state.vscdb`)
> is deliberately not touched.

## Installing

There is no install script: the binary carries the slash commands inside it and
installs itself. Nothing here needs a clone or a Rust toolchain.

**Windows** — double-click `install-antigravity.bat`. It downloads the published
binary and runs `kebacc-antigravity install`, passing on whatever it was given:

```
install-antigravity.bat -AutoSwitch -StatusLine
```

**macOS and Linux** — download the binary for this platform from the newest
`kebacc-antigravity-v*` release and ask it to install itself. The asset is
picked by name off the release list rather than through `releases/latest`,
which points at the Claude half:

```
name=kebacc-antigravity-x86_64-unknown-linux-gnu
url=$(curl -fsSL https://api.github.com/repos/kebab1337420/kebacc-switch/releases |
  grep -o "https://[^\"]*/$name" | head -n 1)
curl -fsSL "$url" -o kebacc-antigravity && chmod +x kebacc-antigravity
./kebacc-antigravity install --auto-switch
```

From a clone it is the same command on the build you just made:

```
cargo build --release -p kebacc-antigravity
target/release/kebacc-antigravity install --auto-switch
```

`--auto-switch` (`-AutoSwitch`) arms the switch, `--status-line`
(`-StatusLine`) points the Claude Code status line here, and `--tools-dir`
(`-ToolsDir`) installs somewhere other than `~/.claude-tools`. Running it again
is how it updates.

## What it does

Inside Claude Code the commands are slash commands:

| Command | What it does |
| --- | --- |
| `/kebacc-add-antigravity` | save the Antigravity login in `~/.gemini/antigravity-cli/antigravity-oauth-token` |
| `/kebacc-list-antigravity` | what is saved, and what is known of each quota |
| `/kebacc-switch-antigravity` | put another saved login in front of Antigravity |
| `/kebacc-remove-antigravity` | forget a saved login; the live session is untouched |
| `/kebacc-auto-antigravity` | arm or disarm the automatic switch |
| `/kebacc-doctor-antigravity` | check the install, the pool and the session hook |
| `/kebacc-update-antigravity` | install the newest release of this half |

From a shell it is the same binary under its own name:

```
kebacc-antigravity list
kebacc-antigravity switch -Email you@example.com
kebacc-antigravity auto
```

## Switching without being asked

`--auto-switch` writes two hooks into `~/.claude/settings.json`, both running
the same `auto`:

- `SessionStart`, so a session that would have opened on a capped account opens
  on a free one instead.
- `PreToolUse`, matching every tool, so an account that runs out *during* a task
  is switched off partway through it rather than at the next session. That one
  reads a stamp file and does nothing at all unless the last check was more than
  twenty seconds ago (`KEBACC_SWITCH_MIDTASK_INTERVAL_MS`).

Both hooks run `kebacc-antigravity` by name, and arming reads and rewrites nothing
else: the Claude switcher, `kebacc-switch`, arms its own pair under its own
binary, and the two never touch each other's. That is what `install` and
`uninstall` call:

```
kebacc-antigravity arm -Provider antigravity -Merge   # add this pool to our hook's scope
kebacc-antigravity arm -Provider antigravity -Drop    # take it out, disarm when nothing is left
```

`-Provider all` is still read as a scope covering this pool, so a hook written
by the older build — where one binary carried both pools — keeps working until
it is rearmed.

## Uninstalling

`kebacc-antigravity uninstall` takes the Antigravity commands out, narrows the
hooks, drops this half's version marker and removes the binary. It asks first;
`--yes` (`-Yes`) answers for you. What the settings name is what it takes back:
a second install of this half, somewhere else on the machine, keeps its own
settings and its own slash commands.

The commands that span both pools — `/kebacc-list-all`, `/kebacc-auto-all` —
go with whichever half leaves last, since they mean nothing on their own.

On Windows a running binary cannot delete itself, so the uninstall leaves a copy
of itself in the temporary directory and that copy takes the name a moment after
this process lets go of it.

The login you are on is left where it is, in the file and in the credential
store both: uninstalling the switcher is not signing out of Antigravity.

The saved logins are kept: they are the point of the tool, and removing the
plugin is not a reason to lose them. `--pool` (`-Pool`) deletes them.

## Where things are kept

| Path | What |
| --- | --- |
| `~/.claude-tools/` | the binary, and `.antigravity-version` — this half's marker |
| `~/.kebacc-switch-antigravity-accounts/` | saved Antigravity logins |
| `~/.claude/commands/kebacc-*-antigravity.md` | the slash commands |
| `~/.gemini/antigravity-cli/antigravity-oauth-token` | the login the Antigravity CLI is using right now |
| `gemini:antigravity` in the OS credential store | the same login, as the IDE reads it |

`KEBACC_SWITCH_ANTIGRAVITY_ACCOUNTS` moves the pool, `ANTIGRAVITY_HOME` is
where the CLI's own file is looked for, and `KEBACC_SWITCH_NO_KEYRING=1` keeps
the credential store out of it.

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
`kebacc-antigravity install` and `kebacc-antigravity uninstall`, in
`crates/kebacc-antigravity/src/cmd/`. The command files above are compiled into
the binary with `include_str!`, so the binary that installs them is the only
thing that has to arrive on the machine, and a command file that CI finds on
disk but not in that list fails the build.

What another switcher sharing the machine has to answer the same way lives in
the same crate: the version markers that say who is installed (`.version` for
the Claude half, `.antigravity-version` for this one), the binary-version guard,
and the scope algebra in `cmd/arm.rs` the session hook is widened and narrowed
by.

---
description: Install the Codex switcher, which lives on its own branch
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*)
---

Run `~/.claude-tools/kebacc-switch`, or `~/.claude-tools/kebacc-switch.exe` on Windows:

```
~/.claude-tools/kebacc-switch install-codex
```

The installer is part of the switcher itself, so there is nothing to download
first and no copy on disk to go stale: an install that predates it updates the
switcher (`/kebacc-update`) and runs this again.

It clones the `Codex` branch, builds `kebacc-codex` with cargo and installs it
into `~/.claude-tools` beside this one: its own binary, its own pool, its own
`*-codex` slash commands, and neither half reads the other's. It needs `git`
and `cargo`, and takes a minute the first time. Nothing on the Claude side is
touched, and running it again is how it updates.

To arm the session-start auto-switch for the Codex pool at the same time, add
`-AutoSwitch` to the call. A checkout on this machine can be used instead of
the clone with `-Source <path>`.

Report the version it installed. The new slash commands appear once Claude Code
is restarted.

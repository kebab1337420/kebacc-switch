---
description: Install the Codex switcher, which lives on its own branch
allowed-tools: Bash(pwsh:*), Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Run:

```
pwsh -NoProfile -File "$HOME/.claude-tools/install-codex.ps1"
```

It clones the `Codex` branch, builds `kebacc-codex` with cargo and installs it
beside kebacc-switch: its own binary, its own `*-codex` slash commands, the same
saved logins. It takes a minute the first time. Nothing of the Claude side is
touched, and running it again is how it updates.

Add `-AutoSwitch` to also arm the session-start auto-switch for the Codex pool.
If the branch has not been pushed yet, pass a local checkout instead:
`-Source <path>`.

Report the version it installed. The new slash commands appear once Claude Code
is restarted.

---
description: Arm the session-start auto-switch for both pools
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*), Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Two pools, two binaries, one session hook each. Run both, `.exe` on Windows:

```
~/.claude-tools/kebacc-switch arm -Provider claude
~/.claude-tools/kebacc-codex arm -Provider codex
```

`kebacc-codex` is a separate plugin. If it is not installed, say so in one line
and arm the Claude pool alone.

This only arms the session-start auto-switch. It never changes the account in
use, whatever the quota says — only `/kebacc-switch-claude` and
`/kebacc-switch-codex` do that. Armed means the next sessions open on an account
that has room.

Print the one line each command prints and stop. Do not offer to switch, do not
read the quota, do not suggest anything else.

---
description: Arm the auto-switch for the Claude Code accounts
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows:

```
~/.claude-tools/kebacc arm -Provider claude
```

This only arms the auto-switch. It never changes the account in use on its own,
whatever the quota says at the moment you run it — only `/kebacc-switch-claude`
does that on the spot.

Armed means two things: the next sessions open on an account that has room, and
a session already running moves to one when the current account runs out
mid-task. It does not wait for Claude to be idle.

Print the one line it prints and stop. Do not offer to switch, do not read the
quota, do not suggest anything else.

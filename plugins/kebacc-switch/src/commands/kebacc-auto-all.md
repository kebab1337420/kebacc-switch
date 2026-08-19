---
description: Arm the auto-switch for both pools, at session start and mid-task
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*), Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Two pools, two binaries, one pair of hooks each. Run both, `.exe` on Windows:

```
~/.claude-tools/kebacc-switch arm -Provider claude
~/.claude-tools/kebacc-codex arm -Provider codex
```

`kebacc-codex` is a separate plugin. If it is not installed, say so in one line
and arm the Claude pool alone.

This only arms the auto-switch. It never changes the account in use on its own,
whatever the quota says at the moment you run it — only `/kebacc-switch-claude`
and `/kebacc-switch-codex` do that on the spot.

Armed means two things: the next sessions open on an account that has room, and
a session already running moves to one when the current account runs out
mid-task. It does not wait for the agent to be idle.

Print the one line each command prints and stop. Do not offer to switch, do not
read the quota, do not suggest anything else.

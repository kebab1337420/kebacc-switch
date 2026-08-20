---
description: Arm the auto-switch for every pool installed, at session start and mid-task
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*), Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*), Bash(~/.claude-tools/kebacc-antigravity:*), Bash(~/.claude-tools/kebacc-antigravity.exe:*)
---

One binary per pool, one pair of hooks each, and this file is the same file
whichever of them installed it. Run every one of these that is on the machine,
`.exe` on Windows:

```
~/.claude-tools/kebacc-switch arm -Provider claude
~/.claude-tools/kebacc-codex arm -Provider codex
~/.claude-tools/kebacc-antigravity arm -Provider antigravity
```

Each is a separate plugin, and a machine can carry any one of them, any two, or
all three. A binary that is not there is not an error: say so in one line and
arm the pools that are.

This only arms the auto-switch. It never changes the account in use on its own,
whatever the quota says at the moment you run it — only `/kebacc-switch-claude`,
`/kebacc-switch-codex` and `/kebacc-switch-antigravity` do that on the spot.

Armed means two things: the next sessions open on an account that has room, and
a session already running moves to one when the current account runs out
mid-task. It does not wait for the agent to be idle.

Print the one line each command prints and stop. Do not offer to switch, do not
read the quota, do not suggest anything else.

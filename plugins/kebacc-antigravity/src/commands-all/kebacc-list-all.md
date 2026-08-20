---
description: Every saved account of every pool installed, fresh numbers and time to reset
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*), Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*), Bash(~/.claude-tools/kebacc-antigravity:*), Bash(~/.claude-tools/kebacc-antigravity.exe:*)
---

One binary per pool, and this file is the same file whichever of them installed
it. Run every one of these that is on the machine, `.exe` on Windows:

```
~/.claude-tools/kebacc-switch list -Provider claude -Refresh -Countdown
~/.claude-tools/kebacc-codex list -Refresh -Countdown
~/.claude-tools/kebacc-antigravity list -Refresh -Countdown
```

Each is a separate plugin, and a machine can carry any one of them, any two, or
all three. A binary that is not there is not an error: say so in one line and
report the pools that are.

The numbers come from the API, not from the cache, and each account shows both
its 5h and its 7d window with the time until each one resets.

One block per pool. Say how many accounts each has, which one is in use (`*`),
and the soonest 5h reset among the capped ones. This only lists: it never
switches, whatever the numbers say.

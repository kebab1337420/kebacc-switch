---
description: Every saved account, Claude and Codex, fresh numbers and time to reset
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*), Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Two pools, two binaries. Run both, `.exe` on Windows:

```
~/.claude-tools/kebacc-switch list -Provider claude -Refresh -Countdown
~/.claude-tools/kebacc-codex list -Refresh -Countdown
```

`kebacc-codex` is a separate plugin. If it is not installed, say so in one line
and report the Claude pool alone.

The numbers come from the API, not from the cache, and each account shows both
its 5h and its 7d window with the time until each one resets.

One block per pool. Say how many accounts each has, which one is in use (`*`),
and the soonest 5h reset among the capped ones. This only lists: it never
switches, whatever the numbers say.

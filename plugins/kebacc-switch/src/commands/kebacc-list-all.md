---
description: Every saved account, fresh numbers and time to reset
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*)
---

Run `~/.claude-tools/kebacc-switch`, or `~/.claude-tools/kebacc-switch.exe` on Windows:

```
~/.claude-tools/kebacc-switch list -Provider all -Refresh -Countdown
```

The numbers come from the API, not from the cache, and each account shows both its 5h and its 7d window with the time until each one resets.

One block per provider. Say how many accounts each has, which one is in use (`*`), and the soonest 5h reset among the capped ones.

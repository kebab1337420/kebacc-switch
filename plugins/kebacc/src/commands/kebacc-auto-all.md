---
description: Arm the auto-switch for every pool, at session start and mid-task
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows:

```
~/.claude-tools/kebacc arm -Provider all
```

This only arms the auto-switch. It never changes the account in use on its own. Print the one line it prints and stop.

---
description: Arm the auto-switch for the Claude Code accounts
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*)
---

Run `~/.claude-tools/kebacc-switch`, or `~/.claude-tools/kebacc-switch.exe` on Windows:

```
~/.claude-tools/kebacc-switch arm -Provider claude
```

This only arms the session-start auto-switch. It never changes the account in
use, whatever the quota says — only `/kebacc-switch-claude` does that. Armed
means the next sessions open on an account that has room.

Print the one line it prints and stop. Do not offer to switch, do not read the
quota, do not suggest anything else.

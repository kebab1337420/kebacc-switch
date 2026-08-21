---
description: Arm the auto-switch for the Antigravity accounts
allowed-tools: Bash(~/.claude-tools/kebacc-antigravity:*), Bash(~/.claude-tools/kebacc-antigravity.exe:*)
---

Run `~/.claude-tools/kebacc-antigravity`, or `~/.claude-tools/kebacc-antigravity.exe` on Windows:

```
~/.claude-tools/kebacc-antigravity arm -Provider antigravity -Merge
```

This only arms the session-start auto-switch for the Antigravity pool. `-Merge` adds
it to whatever this half's hooks already carry, and hooks belonging to another
switcher are not touched. It never changes the account in use, whatever the
quota says — only `/kebacc-switch-antigravity` does that. Armed means the next
sessions open on an account that has room.

Print the one line it prints and stop. Do not offer to switch, do not read the
quota, do not suggest anything else.

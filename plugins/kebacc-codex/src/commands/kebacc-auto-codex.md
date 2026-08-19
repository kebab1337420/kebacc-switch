---
description: Arm the auto-switch for the Codex accounts
allowed-tools: Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Run `~/.claude-tools/kebacc-codex`, or `~/.claude-tools/kebacc-codex.exe` on Windows:

```
~/.claude-tools/kebacc-codex arm -Provider codex -Merge
```

This only arms the session-start auto-switch for the Codex pool. `-Merge` adds
it to whatever this half's hooks already carry, and hooks belonging to another
switcher are not touched. It never changes the account in use, whatever the
quota says — only `/kebacc-switch-codex` does that. Armed means the next
sessions open on an account that has room.

Print the one line it prints and stop. Do not offer to switch, do not read the
quota, do not suggest anything else.

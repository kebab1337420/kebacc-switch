---
description: Arm the auto-switch for the Antigravity accounts
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows:

```
~/.claude-tools/kebacc arm -Provider antigravity -Merge
```

This only arms the session-start auto-switch for the Antigravity pool. `-Merge`
adds it to whatever is already armed, so this command cannot drop Claude or
Codex. It never changes the account in use, whatever the quota says. Only
`/kebacc-switch-antigravity` does that. Armed means the next sessions open on
an account that has room.

Print the one line it prints and stop. Do not offer to switch, do not read the
quota, do not suggest anything else.

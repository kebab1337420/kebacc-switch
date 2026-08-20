---
description: Check the Antigravity install, the pool and the session hook
allowed-tools: Bash(~/.claude-tools/kebacc-antigravity:*), Bash(~/.claude-tools/kebacc-antigravity.exe:*)
---

Run `~/.claude-tools/kebacc-antigravity`, or `~/.claude-tools/kebacc-antigravity.exe` on Windows:

```
~/.claude-tools/kebacc-antigravity doctor -Provider antigravity
```

Report the `!` and `~` lines. Plain-text snapshots are fixed with
`doctor -Provider antigravity -Protect`, unstamped ones with `-Adopt`, and the
credentials from before the last switch come back with `-Rollback`.

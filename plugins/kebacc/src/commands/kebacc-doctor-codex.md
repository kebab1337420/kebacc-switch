---
description: Check the Codex install, the pool and the session hook
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows:

```
~/.claude-tools/kebacc doctor -Provider codex
```

Report the `!` and `~` lines. Plain-text snapshots are fixed with
`doctor -Provider codex -Protect`, unstamped ones with `-Adopt`, and the
credentials from before the last switch come back with `-Rollback`.

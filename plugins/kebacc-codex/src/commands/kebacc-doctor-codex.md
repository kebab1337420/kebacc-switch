---
description: Check the Codex install, the pool and the session hook
allowed-tools: Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Run `~/.claude-tools/kebacc-codex`, or `~/.claude-tools/kebacc-codex.exe` on Windows:

```
~/.claude-tools/kebacc-codex doctor -Provider codex
```

Report the `!` and `~` lines. Plain-text snapshots are fixed with
`doctor -Provider codex -Protect`, unstamped ones with `-Adopt`, and the
credentials from before the last switch come back with `-Rollback`.

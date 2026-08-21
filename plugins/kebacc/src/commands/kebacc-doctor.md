---
description: Check the switcher install and the saved accounts
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows:

```
~/.claude-tools/kebacc doctor -Provider all
```

Report the `!` and `~` lines. Plain-text snapshots are fixed with `doctor -Provider <id> -Protect`, unstamped ones with `-Adopt`.

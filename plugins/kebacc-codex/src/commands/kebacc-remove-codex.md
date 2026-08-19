---
description: Forget a saved Codex account
allowed-tools: Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Run `~/.claude-tools/kebacc-codex`, or `~/.claude-tools/kebacc-codex.exe` on Windows:

```
~/.claude-tools/kebacc-codex remove -Provider codex -Email <email> -Yes
```

Confirm with the user which account before running this: it is not reversible without logging in again. The live session is untouched.

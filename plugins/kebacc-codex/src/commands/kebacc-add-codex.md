---
description: Save the Codex login you are on into the pool
allowed-tools: Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Run `~/.claude-tools/kebacc-codex`, or `~/.claude-tools/kebacc-codex.exe` on Windows:

```
~/.claude-tools/kebacc-codex add -Provider codex
```

Report the account it saved. An API key has no email attached, so if it asks for one, ask the user for it and pass `-Email`.

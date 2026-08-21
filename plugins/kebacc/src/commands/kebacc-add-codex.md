---
description: Save the Codex login you are on into the pool
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows:

```
~/.claude-tools/kebacc add -Provider codex
```

Report the account it saved. An API key has no email attached, so if it asks for one, ask the user for it and pass `-Email`.
